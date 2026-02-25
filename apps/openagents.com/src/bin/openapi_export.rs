use openagents_control_service::openapi::openapi_document;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = openapi_document();
    let encoded = serde_json::to_string(&document)?;
    print!("{encoded}");
    Ok(())
}
