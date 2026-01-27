fn main() -> Result<(), Box<dyn std::error::Error>> {
    let attributes = tauri_build::Attributes::new()
        .codegen(tauri_build::CodegenContext::new());
    tauri_build::try_build(attributes)?;
    Ok(())
}
