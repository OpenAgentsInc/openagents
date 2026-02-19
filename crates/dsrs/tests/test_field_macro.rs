use dsrs::field;
use rstest::*;
use serde_json::json;

#[rstest]
fn test_field_initizalization() {
    let input_field = field! {
        input => input_field : String
    };
    let output_field = field! {
        output => output_field : String
    };
    assert_eq!(input_field["input_field"]["desc"], "");
    assert_eq!(output_field["output_field"]["desc"], "");
}

#[rstest]
fn test_field_macro_single_input() {
    // Test single field definition with input
    let result = field! {
        input["Description of the question"] => question: String
    };

    // Verify the generated JSON structure
    let expected = json!({
        "question": {
            "type": "String",
            "desc": "Description of the question",
            "schema": "",
            "__dsrs_field_type": "input"
        }
    });

    assert_eq!(result, expected);
}

#[rstest]
fn test_field_macro_single_output() {
    // Test single field definition with output
    let result = field! {
        output["The generated answer"] => answer: String
    };

    // Verify the generated JSON structure
    let expected = json!({
        "answer": {
            "type": "String",
            "desc": "The generated answer",
            "schema": "",
            "__dsrs_field_type": "output"
        }
    });

    assert_eq!(result, expected);
}

#[rstest]
fn test_field_macro_multiple_fields() {
    // Test multiple field definitions
    let result = field! {
        input["User's question"] => question: String,
        input["Context information"] => context: String,
        output["Generated answer"] => answer: String,
        output["Confidence score"] => confidence: f64
    };

    // Verify the generated JSON structure
    let expected_question = json!({
        "type": "String",
        "desc": "User's question",
        "schema": "",
        "__dsrs_field_type": "input"
    });

    let expected_context = json!({
        "type": "String",
        "desc": "Context information",
        "schema": "",
        "__dsrs_field_type": "input"
    });

    let expected_answer = json!({
        "type": "String",
        "desc": "Generated answer",
        "schema": "",
        "__dsrs_field_type": "output"
    });

    let expected_confidence = json!({
        "type": "f64",
        "desc": "Confidence score",
        "schema": "",
        "__dsrs_field_type": "output"
    });

    // Check that all fields are present with correct values
    assert_eq!(result["question"], expected_question);
    assert_eq!(result["context"], expected_context);
    assert_eq!(result["answer"], expected_answer);
    assert_eq!(result["confidence"], expected_confidence);
}

#[rstest]
fn test_field_macro_with_different_types() {
    // Test with various field types
    let result = field! {
        input["Boolean flag"] => is_active: bool,
        input["Integer count"] => count: i32,
        input["Text data"] => text: String,
        output["Result vector"] => results: Vec<String>
    };

    // Verify different types are handled correctly
    assert_eq!(result["is_active"]["type"], "bool");
    assert_eq!(result["is_active"]["__dsrs_field_type"], "input");
    assert_eq!(result["is_active"]["desc"], "Boolean flag");

    assert_eq!(result["count"]["type"], "i32");
    assert_eq!(result["count"]["__dsrs_field_type"], "input");
    assert_eq!(result["count"]["desc"], "Integer count");

    assert_eq!(result["text"]["type"], "String");
    assert_eq!(result["text"]["__dsrs_field_type"], "input");
    assert_eq!(result["text"]["desc"], "Text data");

    assert_eq!(result["results"]["type"], "Vec<String>");
    assert_eq!(result["results"]["__dsrs_field_type"], "output");
    assert_eq!(result["results"]["desc"], "Result vector");
}

#[rstest]
fn test_field_macro_usage_in_signature() {
    // Example of how this might be used with a signature
    let input_fields = field! {
        input["The question to answer"] => question: String,
        input["Additional context"] => context: String
    };

    let output_fields = field! {
        output["The answer to the question"] => answer: String
    };

    // These can be used to build signature metadata
    assert!(input_fields["question"].is_object());
    assert!(output_fields["answer"].is_object());
}

#[rstest]
fn test_field_macro_without_description() {
    // Test field definitions without descriptions
    let result = field! {
        input => question: String,
        input => context: String,
        output => answer: String
    };

    // Verify fields are created with empty descriptions
    assert_eq!(result["question"]["type"], "String");
    assert_eq!(result["question"]["__dsrs_field_type"], "input");
    assert_eq!(result["question"]["desc"], "");
    assert_eq!(result["question"]["schema"], "");

    assert_eq!(result["context"]["type"], "String");
    assert_eq!(result["context"]["__dsrs_field_type"], "input");
    assert_eq!(result["context"]["desc"], "");

    assert_eq!(result["answer"]["type"], "String");
    assert_eq!(result["answer"]["__dsrs_field_type"], "output");
    assert_eq!(result["answer"]["desc"], "");
}

#[rstest]
fn test_field_macro_mixed_descriptions() {
    // Test mixing fields with and without descriptions
    let with_desc = field! {
        input["User query"] => question: String,
        output["Response"] => answer: String
    };

    let without_desc = field! {
        input => question: String,
        output => answer: String
    };

    // Verify descriptions are present or empty as expected
    assert_eq!(with_desc["question"]["desc"], "User query");
    assert_eq!(with_desc["question"]["__dsrs_field_type"], "input");
    assert_eq!(with_desc["answer"]["__dsrs_field_type"], "output");
    assert_eq!(with_desc["answer"]["desc"], "Response");

    assert_eq!(without_desc["question"]["desc"], "");
    assert_eq!(without_desc["question"]["__dsrs_field_type"], "input");
    assert_eq!(without_desc["answer"]["__dsrs_field_type"], "output");
    assert_eq!(without_desc["answer"]["desc"], "");
}
