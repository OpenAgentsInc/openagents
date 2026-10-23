fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&schemars::schema_for!(aethyrforge_host::Envelope)).unwrap()
    );
}
