use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=scripts");
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
    let generated = format!(
        "pub fn get(path: &str) -> Option<&'static str> {{\n    match path.replace('\\\\', \"/\").as_str() {{\n{arms}        _ => None,\n    }}\n}}\n"
    );
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("embedded_scripts.rs"),
        generated,
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
