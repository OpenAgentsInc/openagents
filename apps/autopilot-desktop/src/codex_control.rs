use serde_json::{Value, json};

fn build_tool_input_response(params: &Value) -> Value {
    let mut answers = serde_json::Map::new();
    let questions = params
        .get("questions")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    for question in questions {
        let id = question
            .get("id")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());
        let answer = question
            .get("options")
            .and_then(|value| value.as_array())
            .and_then(|options| options.first())
            .and_then(|option| option.get("id"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .unwrap_or_else(|| "yes".to_string());

        if let Some(id) = id {
            answers.insert(
                id,
                json!({
                    "answers": [answer],
                }),
            );
        }
    }

    json!({ "answers": answers })
}

pub(crate) fn build_auto_response(method: &str, params: Option<&Value>) -> Option<Value> {
    match method {
        "execCommandApproval" | "applyPatchApproval" => Some(json!({ "decision": "approved" })),
        "item/tool/requestUserInput" => params.map(build_tool_input_response),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_methods_are_auto_approved() {
        let response = build_auto_response("execCommandApproval", None).expect("response");
        assert_eq!(
            response.get("decision").and_then(Value::as_str),
            Some("approved")
        );
    }

    #[test]
    fn tool_user_input_uses_first_option_or_yes_default() {
        let params = json!({
            "questions": [
                {
                    "id": "q1",
                    "options": [
                        {"id": "choice-a"},
                        {"id": "choice-b"}
                    ]
                },
                {
                    "id": "q2",
                    "options": []
                }
            ]
        });
        let response =
            build_auto_response("item/tool/requestUserInput", Some(&params)).expect("response");
        assert_eq!(
            response
                .get("answers")
                .and_then(|value| value.get("q1"))
                .and_then(|value| value.get("answers"))
                .and_then(|value| value.get(0))
                .and_then(Value::as_str),
            Some("choice-a")
        );
        assert_eq!(
            response
                .get("answers")
                .and_then(|value| value.get("q2"))
                .and_then(|value| value.get("answers"))
                .and_then(|value| value.get(0))
                .and_then(Value::as_str),
            Some("yes")
        );
    }
}
