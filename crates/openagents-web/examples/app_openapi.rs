fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&openagents_web::app_openapi::document()).unwrap()
    );
}
