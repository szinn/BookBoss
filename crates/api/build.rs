fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::compile_protos("proto/system.proto")?;

    println!("cargo:rerun-if-changed=proto");

    Ok(())
}
