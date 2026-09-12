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
    let index = fs::read_to_string(art_root.join("index.tsv")).unwrap()
        + &fs::read_to_string(art_root.join("directions.tsv")).unwrap();
    let art = index
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
    let battle_root = Path::new(&manifest).join("assets/wesnoth/battle");
    println!("cargo:rerun-if-changed={}", battle_root.display());
    let battle_art = fs::read_to_string(battle_root.join("index.tsv"))
        .unwrap()
        .lines()
        .map(|line| {
            let (id, relative) = line.split_once('\t').unwrap();
            format!(
                "({:?}, include_bytes!({:?}).as_slice()),\n",
                id,
                battle_root.join(relative).to_string_lossy()
            )
        })
        .collect::<String>();
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("embedded_battle_art.rs"),
        format!("pub static ALL: &[(&str, &[u8])] = &[\n{battle_art}];\n"),
    )
    .unwrap();
    let mut village_art = String::new();
    for directory in ["flags", "terrain/village"] {
        let root = Path::new(&manifest).join("assets/wesnoth").join(directory);
        println!("cargo:rerun-if-changed={}", root.display());
        let mut paths = fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let id = format!(
                "{directory}/{}",
                path.file_stem().unwrap().to_str().unwrap()
            );
            village_art.push_str(&format!(
                "({id:?}, include_bytes!({:?}).as_slice()),\n",
                path.to_string_lossy()
            ));
        }
    }
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("embedded_village_art.rs"),
        format!("pub static ALL: &[(&str, &[u8])] = &[\n{village_art}];\n"),
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
