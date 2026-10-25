use std::{fs,path::Path};
fn hash_tree(path:&Path,hash:&mut u64) {
    let mut entries:Vec<_>=fs::read_dir(path).unwrap().map(|e|e.unwrap().path()).collect();entries.sort();
    for entry in entries {
        if entry.is_dir(){hash_tree(&entry,hash);}else if matches!(entry.extension().and_then(|s|s.to_str()),Some("rs"|"cs"|"toml")) {
            println!("cargo:rerun-if-changed={}",entry.display());
            for byte in fs::read(entry).unwrap(){*hash=(*hash^u64::from(byte)).wrapping_mul(0x100000001b3);}
        }
    }
}
fn main() {
    let mut hash=0xcbf29ce484222325;
    hash_tree(Path::new("src"),&mut hash);hash_tree(Path::new("../src"),&mut hash);
    hash_tree(Path::new("vendor/hudhook/src"),&mut hash);
    hash_tree(Path::new("vendor/fromsoftware-rs/crates"),&mut hash);
    for file in ["build.rs","Cargo.toml","Cargo.lock"] {
        println!("cargo:rerun-if-changed={file}");
        for byte in fs::read(file).unwrap(){hash=(hash^u64::from(byte)).wrapping_mul(0x100000001b3);}
    }
    println!("cargo:rustc-env=TOWER_BUILD_ID={hash:016x}");
}
