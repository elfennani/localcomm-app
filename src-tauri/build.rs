fn main() -> Result<(), Box<dyn std::error::Error>> {
    tauri_build::build();
    tonic_prost_build::compile_protos("proto/localcomm.proto")?;

    Ok(())
}
