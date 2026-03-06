use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    tonic_prost_build::configure()
        .file_descriptor_set_path(out_dir.join("system_descriptor.bin"))
        .compile_protos(&["proto/system.proto"], &["proto"])?;

    println!("cargo:rerun-if-changed=proto");

    Ok(())
}
