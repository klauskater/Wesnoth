//! Immutable access to a replaceable game package.
//!
//! Paths are logical package paths; game semantics stay in Lua. Contract:
//! `contracts/target/modules/engine/resources.md`.

use std::{collections::BTreeMap, path::Component};

use serde::{Deserialize, Serialize};

use super::data_format::{self, Node};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub package_id: String,
    pub package_version: String,
    pub entry: String,
    pub resource_roots: Vec<String>,
    pub protocol_version: u32,
    pub modules: Vec<String>,
    #[serde(default)]
    pub assets: BTreeMap<String, AssetDescriptor>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct AssetDescriptor {
    pub path: String,
    pub media_type: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct PackageIdentity {
    pub package_id: String,
    pub package_version: String,
    pub protocol_version: u32,
}

impl Manifest {
    pub fn identity(&self) -> PackageIdentity {
        PackageIdentity {
            package_id: self.package_id.clone(),
            package_version: self.package_version.clone(),
            protocol_version: self.protocol_version,
        }
    }
}

pub struct Package<'a> {
    manifest: Manifest,
    read: &'a dyn Fn(&str) -> Result<String, String>,
}

impl<'a> Package<'a> {
    pub fn open(read: &'a impl Fn(&str) -> Result<String, String>) -> Result<Self, String> {
        let source = read("package.json")
            .map_err(|error| format!("cannot read package manifest: {error}"))?;
        let manifest: Manifest = serde_json::from_str(&source)
            .map_err(|error| format!("invalid package manifest: {error}"))?;
        validate_manifest(&manifest)?;
        Ok(Self { manifest, read })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn read_text(&self, path: &str) -> Result<String, String> {
        let path = normalize_path(path)?;
        if !self
            .manifest
            .resource_roots
            .iter()
            .any(|root| path == *root || path.starts_with(&format!("{root}/")))
        {
            return Err(format!("resource path is outside declared roots: {path}"));
        }
        (self.read)(&path)
    }

    pub fn read_data(&self, path: &str) -> Result<Vec<Node>, String> {
        data_format::parse(&self.read_text(path)?).map_err(|error| format!("{path}: {error}"))
    }

    pub fn module(&self, name: &str) -> Result<String, String> {
        if !self.manifest.modules.iter().any(|module| module == name) {
            return Err(format!("unknown package module: {name}"));
        }
        self.read_text(&module_path(name)?)
    }

    pub fn asset(&self, id: &str) -> Result<&AssetDescriptor, String> {
        self.manifest
            .assets
            .get(id)
            .ok_or_else(|| format!("unknown package asset: {id}"))
    }

    pub fn module_sources(&self) -> Result<BTreeMap<String, String>, String> {
        self.manifest
            .modules
            .iter()
            .map(|name| Ok((name.clone(), self.module(name)?)))
            .collect()
    }
}

fn validate_manifest(manifest: &Manifest) -> Result<(), String> {
    if manifest.package_id.trim().is_empty()
        || manifest.package_version.trim().is_empty()
        || manifest.protocol_version == 0
        || manifest.resource_roots.is_empty()
        || manifest.modules.is_empty()
    {
        return Err("package manifest has empty required fields".into());
    }
    if !manifest
        .modules
        .iter()
        .any(|module| module == &manifest.entry)
    {
        return Err("package entry is not declared in modules".into());
    }
    for root in &manifest.resource_roots {
        let normalized = normalize_path(root)?;
        if normalized != *root || root.contains('/') {
            return Err(format!("resource root must be one path segment: {root}"));
        }
    }
    for module in &manifest.modules {
        module_path(module)?;
    }
    for (id, asset) in &manifest.assets {
        if id.trim().is_empty() || asset.media_type.trim().is_empty() {
            return Err("asset id and media_type must not be empty".into());
        }
        let path = normalize_path(&asset.path)?;
        if path != asset.path
            || !manifest
                .resource_roots
                .iter()
                .any(|root| path == *root || path.starts_with(&format!("{root}/")))
        {
            return Err(format!("asset path is outside declared roots: {path}"));
        }
    }
    Ok(())
}

fn module_path(name: &str) -> Result<String, String> {
    if name.is_empty()
        || name.split('.').any(|segment| {
            segment.is_empty()
                || !segment
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
        })
    {
        return Err(format!("invalid module name: {name}"));
    }
    Ok(format!("{}.lua", name.replace('.', "/")))
}

fn normalize_path(path: &str) -> Result<String, String> {
    let path = path.replace('\\', "/");
    let parsed = std::path::Path::new(&path);
    if path.is_empty()
        || parsed.is_absolute()
        || parsed.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!("invalid package path: {path}"));
    }
    let normalized = parsed
        .components()
        .filter_map(|component| match component {
            Component::Normal(segment) => segment.to_str(),
            Component::CurDir => None,
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    if normalized.is_empty() {
        return Err(format!("invalid package path: {path}"));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package() -> (BTreeMap<String, String>, String) {
        let manifest = serde_json::json!({
            "package_id": "test",
            "package_version": "1",
            "entry": "game.init",
            "resource_roots": ["game", "rules", "scenarios"],
            "protocol_version": 1,
            "modules": ["game.init", "rules.example"],
            "assets": {
                "ui.banner": {"path": "game/banner.png", "media_type": "image/png"}
            }
        })
        .to_string();
        let files = BTreeMap::from([
            ("package.json".into(), manifest.clone()),
            ("game/init.lua".into(), "return {}".into()),
            ("rules/example.lua".into(), "return {}".into()),
            ("scenarios/test.wml".into(), "[unknown]\n[/unknown]".into()),
        ]);
        (files, manifest)
    }

    #[test]
    fn package_rejects_escape_and_unknown_modules() {
        let (files, _) = package();
        let read = |path: &str| files.get(path).cloned().ok_or_else(|| path.to_owned());
        let package = Package::open(&read).unwrap();
        assert!(package.read_text("../package.json").is_err());
        assert!(package.read_text("Cargo.toml").is_err());
        assert!(package.module("rules.missing").is_err());
        assert_eq!(package.asset("ui.banner").unwrap().path, "game/banner.png");
        assert!(package.asset("ui.missing").is_err());
        assert_eq!(
            package.read_data("scenarios/test.wml").unwrap()[0].name,
            "unknown"
        );
    }
}
