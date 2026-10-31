// Generates `include_bytes!` tables for the key art and the icons in `launcher/assets`, so the binary
// embeds every image and never reads the working directory at runtime. The directories are listed
// at build time and the build reruns when a file in them changes, so replaced or added art only
// needs a rebuild (REDESIGN.md "Art": new renders replace the files, no code change).
use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let assets = manifest.join("../../assets");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    // Forward slashes work for include_bytes! on every platform; escape for the string literal.
    let lit = |p: &PathBuf| p.to_string_lossy().replace('\\', "/");

    // ("<asset path>", file): every image under art/{hero,card}/, art/shade-hero.png, the icons
    // and the demo avatars, sorted by asset path for binary search.
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for (dir, exts) in [
        ("art/hero", &["jpg", "png"][..]),
        ("art/card", &["jpg", "png"][..]),
        ("icons", &["svg"][..]),
        ("demo/avatars", &["png"][..]),
    ] {
        let d = assets.join(dir);
        println!("cargo:rerun-if-changed={}", d.display());
        for e in fs::read_dir(&d).unwrap().filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().into_owned();
            if exts.iter().any(|x| name.ends_with(&format!(".{x}"))) {
                println!("cargo:rerun-if-changed={}", e.path().display());
                files.push((format!("{dir}/{name}"), e.path()));
            }
        }
    }
    let shade = assets.join("art/shade-hero.png");
    println!("cargo:rerun-if-changed={}", shade.display());
    files.push(("art/shade-hero.png".into(), shade));
    files.sort();

    let mut src = format!("pub static FILES: [(&str, &[u8]); {}] = [\n", files.len());
    for (key, p) in &files {
        src.push_str(&format!(
            "    (\"{key}\", include_bytes!(\"{}\")),\n",
            lit(p)
        ));
    }
    src.push_str("];\n");
    fs::write(out_dir.join("files.rs"), src).unwrap();

    // Settings' About names the commit of a CI build (`SRL_COMMIT`). A local build says it is
    // one instead: its working tree may not match any commit.
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    if let Ok(sha) = env::var("GITHUB_SHA")
        && sha.len() >= 7
    {
        println!("cargo:rustc-env=SRL_COMMIT={}", &sha[..7]);
    }
}
