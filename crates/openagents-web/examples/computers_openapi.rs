fn ordered(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(values) => {
            let values: std::collections::BTreeMap<_, _> = values
                .into_iter()
                .map(|(key, value)| (key, ordered(value)))
                .collect();
            serde_json::Value::Object(values.into_iter().collect())
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(ordered).collect())
        }
        value => value,
    }
}

fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&ordered(openagents_web::app_openapi::computers_document()))
            .unwrap()
    );
}
