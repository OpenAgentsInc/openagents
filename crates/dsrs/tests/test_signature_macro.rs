use dsrs::Signature;
use rstest::*;
use schemars::JsonSchema;

#[Signature(cot, hint)]
struct TestSignature {
    /// This is a test instruction
    /// What is the meaning of life?
    #[input(desc = "The main question to answer")]
    question: String,

    #[input(desc = "Additional context for the question")]
    context: String,

    #[output(desc = "The answer to the question")]
    answer: Vec<i8>,

    #[output(desc = "Confidence score")]
    confidence: f32,
}

#[allow(dead_code)]
#[derive(JsonSchema)]
struct TestOutput {
    output1: i8,
    output2: String,
    output3: bool,
}

#[Signature]
struct TestSignature2 {
    /// This is a test input
    ///
    /// What is the meaning of life?

    #[input(desc = "The first input")]
    input1: String,

    #[input(desc = "The second input")]
    input2: i8,

    #[output]
    output1: TestOutput,
}

#[rstest]
fn test_signature_macro() {
    let signature = TestSignature::new();
    let expected_schema = serde_json::to_value(schemars::schema_for!(Vec<i8>)).unwrap();

    assert_eq!(
        signature.instruction,
        "This is a test instruction\nWhat is the meaning of life?"
    );
    assert_eq!(signature.input_fields["question"]["type"], "String");
    assert_eq!(
        signature.input_fields["question"]["desc"],
        "The main question to answer"
    );
    assert_eq!(signature.input_fields["question"]["schema"], "");
    assert_eq!(signature.input_fields["context"]["type"], "String");
    assert_eq!(
        signature.input_fields["context"]["desc"],
        "Additional context for the question"
    );
    assert_eq!(signature.input_fields["context"]["schema"], "");
    assert_eq!(signature.output_fields["answer"]["type"], "Vec < i8 >");
    assert_eq!(
        signature.output_fields["answer"]["desc"],
        "The answer to the question"
    );
    assert_eq!(signature.output_fields["answer"]["schema"], expected_schema);
    assert_eq!(signature.output_fields["reasoning"]["type"], "String");
    assert_eq!(
        signature.output_fields["reasoning"]["desc"],
        "Think step by step"
    );
    assert_eq!(signature.output_fields["reasoning"]["schema"], "");
    assert_eq!(signature.output_fields["confidence"]["type"], "f32");
    assert_eq!(
        signature.output_fields["confidence"]["desc"],
        "Confidence score"
    );
    assert_eq!(signature.output_fields["confidence"]["schema"], "");
    assert_eq!(signature.input_fields["hint"]["type"], "String");
    assert_eq!(signature.input_fields["hint"]["desc"], "Hint for the query");
    assert_eq!(signature.input_fields["hint"]["schema"], "");

    let signature = TestSignature2::new();

    assert_eq!(
        signature.instruction,
        "This is a test input\n\nWhat is the meaning of life?"
    );
    assert_eq!(signature.input_fields["input1"]["type"], "String");
    assert_eq!(signature.input_fields["input1"]["desc"], "The first input");
    assert_eq!(signature.input_fields["input1"]["schema"], "");
    assert_eq!(signature.input_fields["input2"]["type"], "i8");
    assert_eq!(signature.input_fields["input2"]["desc"], "The second input");
    assert_eq!(signature.input_fields["input2"]["schema"], "");
    assert_eq!(signature.output_fields["output1"]["type"], "TestOutput");
    assert_eq!(signature.output_fields["output1"]["desc"], "");
    let expected_schema = serde_json::to_value(schemars::schema_for!(TestOutput)).unwrap();
    assert_eq!(
        signature.output_fields["output1"]["schema"],
        expected_schema["properties"]
    );
}
