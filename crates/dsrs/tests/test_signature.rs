use dsrs::{MetaSignature, field, sign};
use rstest::*;

#[rstest]
fn test_signature_from_string() {
    let signature = sign! {
        (inp1: String, inp2: String) -> out1: String, out2: String
    };

    assert_eq!(signature.instruction, "");
    assert_eq!(signature.input_fields_len(), 2);
    assert_eq!(signature.output_fields_len(), 2);
}

#[rstest]
fn test_signature_append() {
    let mut signature = sign! {
        (inp1: String, inp2: String) -> out1: String, out2: String
    };
    let field_obj = field! {
        input => inp3 : String
    };
    let _ = signature.append("inp3", field_obj["inp3"].clone());

    assert_eq!(signature.input_fields_len(), 3);
    assert_eq!(
        signature.input_fields.get("inp3").unwrap()["__dsrs_field_type"],
        "input"
    );
    assert_eq!(signature.input_fields.get("inp3").unwrap()["desc"], "");
    assert_eq!(
        signature.input_fields.get("inp1").unwrap()["__dsrs_field_type"],
        "input"
    );
    assert_eq!(signature.input_fields.get("inp1").unwrap()["desc"], "");
}
