// Generates `include_bytes!` tables for the 500 avatar PNGs, the Lucide icons and the key art, so the binary embeds
// every asset and never touches the working directory at runtime.
use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let assets = manifest.join("..").join("assets");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    // Forward slashes work for include_bytes! on every platform; escape for the string literal.
    let lit = |p: &PathBuf| p.to_string_lossy().replace('\\', "/");

    let avatars = assets.join("avatars");
    println!("cargo:rerun-if-changed={}", avatars.display());
    let mut src = String::from("pub static AVATARS: [&[u8]; 500] = [\n");
    for i in 0..500 {
        let p = avatars.join(format!("{i:03}.png"));
        assert!(p.exists(), "missing avatar {}", p.display());
        src.push_str(&format!("    include_bytes!(\"{}\"),\n", lit(&p)));
    }
    src.push_str("];\n");
    fs::write(out_dir.join("avatars.rs"), src).unwrap();

    // Icons: `assets/icons/<name>.svg` -> ("<name>", bytes), sorted by name for binary search.
    let icons = assets.join("icons");
    println!("cargo:rerun-if-changed={}", icons.display());
    let mut names: Vec<String> = fs::read_dir(&icons)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".svg").map(str::to_owned)
        })
        .collect();
    names.sort();
    let mut src = format!("pub static ICONS: [(&str, &[u8]); {}] = [\n", names.len());
    for name in &names {
        let p = icons.join(format!("{name}.svg"));
        src.push_str(&format!(
            "    (\"{name}\", include_bytes!(\"{}\")),\n",
            lit(&p)
        ));
    }
    src.push_str("];\n");
    fs::write(out_dir.join("icons.rs"), src).unwrap();

    // Key art (REDESIGN.md): every file under `assets/art/{hero,card}/` plus `art/shade-hero.png`,
    // as ("art/<dir>/<file>", bytes), sorted for binary search. The directories are scanned at
    // build time, so art replaced or added under the same names is picked up by a rebuild.
    let art = assets.join("art");
    println!("cargo:rerun-if-changed={}", art.display());
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for dir in ["hero", "card"] {
        let d = art.join(dir);
        println!("cargo:rerun-if-changed={}", d.display());
        for e in fs::read_dir(&d).unwrap().filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.ends_with(".jpg") || name.ends_with(".png") {
                println!("cargo:rerun-if-changed={}", e.path().display());
                files.push((format!("art/{dir}/{name}"), e.path()));
            }
        }
    }
    let shade = art.join("shade-hero.png");
    println!("cargo:rerun-if-changed={}", shade.display());
    files.push(("art/shade-hero.png".into(), shade));
    files.sort();
    let mut src = format!("pub static ART: [(&str, &[u8]); {}] = [\n", files.len());
    for (key, p) in &files {
        src.push_str(&format!(
            "    (\"{key}\", include_bytes!(\"{}\")),\n",
            lit(p)
        ));
    }
    src.push_str("];\n");
    fs::write(out_dir.join("art.rs"), src).unwrap();
}
