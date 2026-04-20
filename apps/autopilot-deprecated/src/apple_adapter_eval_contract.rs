use psionic_eval::AppleAdapterObservedSampleOutput;
use serde_json::{Value, json};

pub(crate) fn runtime_error_observed_output(
    sample_id: &str,
    error: String,
    structured: bool,
) -> AppleAdapterObservedSampleOutput {
    let mut observed = AppleAdapterObservedSampleOutput::from_text(
        sample_id.to_string(),
        format!("runtime_error:{error}"),
    );
    observed
        .metadata
        .insert(String::from("runtime_error"), Value::String(error.clone()));
    observed.metadata.insert(
        String::from("apple_adapter.runtime_failures"),
        json!([{
            "failure_code": "bridge_request_failed",
            "detail": error,
        }]),
    );
    if structured {
        if let Some(failure_code) = structured_contract_failure_code(
            observed
                .metadata
                .get("runtime_error")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        ) {
            observed.metadata.insert(
                String::from("apple_adapter.structured_contract_failures"),
                json!([{
                    "failure_class": "harness_contract",
                    "failure_code": failure_code,
                    "detail": observed
                        .metadata
                        .get("runtime_error")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                }]),
            );
        }
        observed = observed.with_structured_output(Value::Null);
    }
    observed
}

fn structured_contract_failure_code(error: &str) -> Option<&'static str> {
    let normalized = error.to_ascii_lowercase();
    if normalized.contains("failed to deserialize a generable type from model output")
        || normalized.contains("decodingfailure")
        || normalized.contains("decoding_failure")
        || normalized.contains("failed to decode apple fm structured content")
    {
        return Some("typed_deserialization_failed");
    }
    if normalized.contains("invalid apple fm generation schema")
        || normalized.contains("invalid_generation_schema")
        || normalized.contains("invalid generation schema")
    {
        return Some("invalid_generation_schema");
    }
    if normalized.contains("structured request failed") {
        return Some("structured_request_failed");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::runtime_error_observed_output;

    #[test]
    fn structured_runtime_errors_tag_contract_failures() {
        let observed = runtime_error_observed_output(
            "sample-structured",
            String::from(
                "Foundation Models structured request failed: Failed to deserialize a Generable type from model output",
            ),
            true,
        );
        let failures = observed
            .metadata
            .get("apple_adapter.structured_contract_failures")
            .and_then(serde_json::Value::as_array)
            .expect("structured contract failures");
        assert_eq!(
            failures[0]
                .get("failure_code")
                .and_then(serde_json::Value::as_str),
            Some("typed_deserialization_failed")
        );
    }
}
