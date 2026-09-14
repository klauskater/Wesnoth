//! Syntax-only parsing for declarative game-package data.
//!
//! Unknown game tags remain valid; semantic typing belongs to Lua. Contract:
//! `contracts/target/modules/engine/data_format.md`.

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub attributes: BTreeMap<String, String>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn attribute(&self, name: &str) -> Result<&str, String> {
        self.attributes
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| format!("[{}] has no required attribute {name}", self.name))
    }

    pub fn child(&self, name: &str) -> Result<&Node, String> {
        self.children
            .iter()
            .find(|child| child.name == name)
            .ok_or_else(|| format!("[{}] has no required child [{name}]", self.name))
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |child| child.name == name)
    }
}

pub fn parse(input: &str) -> Result<Vec<Node>, String> {
    let lines: Vec<&str> = input.lines().collect();
    let mut roots = Vec::new();
    let mut stack: Vec<Node> = Vec::new();
    let mut line_index = 0;

    while line_index < lines.len() {
        let line_number = line_index + 1;
        let line = lines[line_index].trim();
        line_index += 1;

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(name) = line.strip_prefix("[/").and_then(|v| v.strip_suffix(']')) {
            validate_name(name, "tag", line_number)?;
            let node = stack
                .pop()
                .ok_or_else(|| format!("line {line_number}: unexpected closing tag [{line}]"))?;
            if node.name != name {
                return Err(format!(
                    "line {line_number}: expected closing tag [/{0}], got [/{name}]",
                    node.name
                ));
            }
            if let Some(parent) = stack.last_mut() {
                parent.children.push(node);
            } else {
                roots.push(node);
            }
            continue;
        }

        if let Some(name) = line.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            validate_name(name, "tag", line_number)?;
            stack.push(Node {
                name: name.to_owned(),
                attributes: BTreeMap::new(),
                children: Vec::new(),
            });
            continue;
        }

        let (key, raw_value) = line
            .split_once('=')
            .ok_or_else(|| format!("line {line_number}: expected key=value, got {line}"))?;
        let current = stack
            .last_mut()
            .ok_or_else(|| format!("line {line_number}: attribute outside a tag"))?;
        let key = key.trim();
        validate_name(key, "attribute", line_number)?;
        let mut value = raw_value.trim().to_owned();

        if value == "<<" {
            let mut parts = Vec::new();
            loop {
                let next = lines
                    .get(line_index)
                    .ok_or_else(|| format!("line {line_number}: unclosed << value"))?;
                line_index += 1;
                if next.trim() == ">>" {
                    break;
                }
                parts.push(*next);
            }
            value = parts.join("\n").trim().to_owned();
        } else if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            value = value[1..value.len() - 1].to_owned();
        }

        if current.attributes.insert(key.to_owned(), value).is_some() {
            return Err(format!("line {line_number}: duplicate attribute {key}"));
        }
    }

    if let Some(node) = stack.last() {
        return Err(format!("unclosed tag [{}]", node.name));
    }
    Ok(roots)
}

fn validate_name(name: &str, kind: &str, line: usize) -> Result<(), String> {
    if name.is_empty()
        || name.trim() != name
        || name
            .chars()
            .any(|character| character.is_whitespace() || matches!(character, '[' | ']' | '='))
    {
        return Err(format!("line {line}: invalid {kind} name: {name:?}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_and_multiline_values() {
        let nodes = parse("[map]\nwidth=2\ndata=<<\na,b\nc,d\n>>\n[/map]").unwrap();
        assert_eq!(nodes[0].attribute("width").unwrap(), "2");
        assert_eq!(nodes[0].attribute("data").unwrap(), "a,b\nc,d");
    }

    #[test]
    fn rejects_mismatched_tags() {
        assert!(parse("[a]\n[/b]").unwrap_err().contains("expected closing"));
    }

    #[test]
    fn rejects_malformed_structure() {
        for source in [
            "[]\n[/]",
            "[a]\n=x\n[/a]",
            "key=value",
            "[a]\nvalue=<<\nmissing end\n[/a]",
            "[a]\nkey=one\nkey=two\n[/a]",
            "[a]",
        ] {
            assert!(parse(source).is_err(), "must reject {source:?}");
        }
    }
}
