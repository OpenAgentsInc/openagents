fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&lab_api::contract::document()).unwrap()
    );
}
