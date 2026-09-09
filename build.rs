use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=scripts");
    println!("cargo:rerun-if-changed=assets/wesnoth/core-units");
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let scripts = Path::new(&manifest).join("scripts");
    let mut files = Vec::new();
    collect(&scripts, &mut files);
    files.sort();

    let arms = files
        .iter()
        .map(|path| {
            let relative = path
                .strip_prefix(&scripts)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            format!(
                "        {:?} => Some(include_str!({:?})),\n",
                relative,
                path.to_string_lossy()
            )
        })
        .collect::<String>();
    let paths = files
        .iter()
        .map(|path| {
            let relative = path
                .strip_prefix(&scripts)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            format!("    {:?},\n", relative)
        })
        .collect::<String>();
    let generated = format!(
        "pub static PATHS: &[&str] = &[\n{paths}];\n\npub fn paths<'a>(prefix: &'a str) -> impl Iterator<Item = &'static str> + 'a {{\n    PATHS.iter().copied().filter(move |path| path.starts_with(prefix))\n}}\n\npub fn get(path: &str) -> Option<&'static str> {{\n    match path.replace('\\\\', \"/\").as_str() {{\n{arms}        _ => None,\n    }}\n}}\n"
    );
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("embedded_scripts.rs"),
        generated,
    )
    .unwrap();

    let art_root = Path::new(&manifest).join("assets/wesnoth/core-units");
    let art = fs::read_to_string(art_root.join("index.tsv"))
        .unwrap()
        .lines()
        .map(|line| {
            let (id, relative) = line.split_once('\t').unwrap();
            let path = art_root.join(relative);
            format!(
                "        ({:?}, include_bytes!({:?}).as_slice()),\n",
                id,
                path.to_string_lossy()
            )
        })
        .collect::<String>();
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("embedded_unit_art.rs"),
        format!("pub static ALL: &[(&str, &[u8])] = &[\n{art}];\n"),
    )
    .unwrap();
}

fn collect(directory: &Path, files: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, files);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "wml" || extension == "lua")
        {
            files.push(path);
        }
    }
}
