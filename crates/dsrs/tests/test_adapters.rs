use schemars::JsonSchema;
use std::sync::Arc;
use tokio::sync::Mutex;

use dsrs::{
    Cache, Chat, ChatAdapter, DummyLM, Example, Message, MetaSignature, Signature,
    adapter::Adapter, example, hashmap, sign,
};

#[tokio::test]
#[cfg_attr(miri, ignore)]
async fn test_chat_adapter() {
    let signature = sign! {
        (problem: String) -> answer: String
    };

    let lm = DummyLM::default();
    let adapter = ChatAdapter;

    let messages: Chat = adapter.format(
        &signature,
        Example::new(
            hashmap! {
                "problem".to_string() => "What is the capital of France?".to_string().into(),
                "answer".to_string() => "Paris".to_string().into(),
            },
            vec!["problem".to_string()],
            vec!["answer".to_string()],
        ),
    );

    let json_value = messages.to_json();
    let json = json_value.as_array().unwrap();

    assert_eq!(messages.len(), 2);
    assert_eq!(json[0]["role"], "system");
    assert_eq!(json[1]["role"], "user");

    assert_eq!(
        json[0]["content"],
        "Your input fields are:\n1. `problem` (String)\n\nYour output fields are:\n1. `answer` (String)\n\nAll interactions will be structured in the following way, with the appropriate values filled in.\n\n[[ ## problem ## ]]\nproblem\n\n[[ ## answer ## ]]\nanswer\n\n[[ ## completed ## ]]\n\nIn adhering to this structure, your objective is:\n\tGiven the fields `problem`, produce the fields `answer`."
    );
    assert_eq!(
        json[1]["content"],
        "[[ ## problem ## ]]\nWhat is the capital of France?\n\nRespond with the corresponding output fields, starting with the field `answer`, and then ending with the marker for `completed`.".to_string()
    );

    let test_example = example! {
        "problem": "input" => "What is the capital of France?",
        "answer": "output" => "Paris"
    };
    let response = lm
        .call(
            test_example,
            Chat::new(vec![
                Message::system("You are a helpful assistant."),
                Message::user("Hello, world!"),
            ]),
            "[[ ## answer ## ]]\n150 degrees\n\n[[ ## completed ## ]]".to_string(),
        )
        .await
        .unwrap();
    let output = adapter.parse_response(&signature, response.output);

    assert_eq!(output.len(), 1);
    assert_eq!(output.get("answer").unwrap(), "150 degrees");
}

#[allow(dead_code)]
#[Signature(cot, hint)]
struct TestSignature {
    ///You are a helpful assistant that can answer questions. You will be given a problem and a hint. You will need to use the hint to answer the problem. You will then need to provide the reasoning and the answer.

    #[input]
    pub problem: String,
    #[output]
    pub answer: String,
}

#[tokio::test]
#[cfg_attr(miri, ignore)]
async fn test_chat_adapter_with_multiple_fields() {
    let signature = TestSignature::new();

    let lm = DummyLM::default();
    let adapter = ChatAdapter;

    let messages: Chat = adapter.format(
        &signature,
        Example::new(
            hashmap! {
                "problem".to_string() => "What is the capital of France?".to_string().into(),
                "hint".to_string() => "The capital of France is Paris.".to_string().into(),
            },
            vec!["problem".to_string(), "hint".to_string()],
            vec!["reasoning".to_string(), "answer".to_string()],
        ),
    );

    let json_value = messages.to_json();
    let json = json_value.as_array().unwrap();

    assert_eq!(messages.len(), 2);
    assert_eq!(json[0]["role"], "system");
    assert_eq!(json[1]["role"], "user");

    assert_eq!(
        json[0]["content"],
        "Your input fields are:\n1. `problem` (String)\n2. `hint` (String): Hint for the query\n\nYour output fields are:\n1. `reasoning` (String): Think step by step\n2. `answer` (String)\n\nAll interactions will be structured in the following way, with the appropriate values filled in.\n\n[[ ## problem ## ]]\nproblem\n\n[[ ## hint ## ]]\nhint\n\n[[ ## reasoning ## ]]\nreasoning\n\n[[ ## answer ## ]]\nanswer\n\n[[ ## completed ## ]]\n\nIn adhering to this structure, your objective is:\n\tYou are a helpful assistant that can answer questions. You will be given a problem and a hint. You will need to use the hint to answer the problem. You will then need to provide the reasoning and the answer.".to_string()
    );
    assert_eq!(
        json[1]["content"],
        "[[ ## problem ## ]]\nWhat is the capital of France?\n\n[[ ## hint ## ]]\nThe capital of France is Paris.\n\nRespond with the corresponding output fields, starting with the field `reasoning`, then `answer`, and then ending with the marker for `completed`."
    );

    let test_example = example! {
        "problem": "input" => "What is the capital of France?",
        "hint": "output" => "The capital of France is Paris.",
        "reasoning": "output" => "The capital of France is Paris.",
        "answer": "output" => "Paris"
    };

    let response = lm
        .call(
            test_example,
            Chat::new(vec![
                Message::system("You are a helpful assistant."),
                Message::user("Hello, world!"),
            ]),
            "[[ ## reasoning ## ]]\nThe capital of France is Paris.\n\n[[ ## answer ## ]]\nParis\n\n[[ ## completed ## ]]".to_string(),
        )
        .await
        .unwrap();
    let output = adapter.parse_response(&signature, response.output);

    assert_eq!(output.len(), 2);
    assert_eq!(
        output.get("reasoning").unwrap(),
        "The capital of France is Paris."
    );
    assert_eq!(output.get("answer").unwrap(), "Paris");
}

#[allow(dead_code)]
#[derive(JsonSchema)]
struct TestOutput {
    pub reasoning: String,
    pub rating: i8,
}

#[allow(dead_code)]
#[Signature]
struct TestSignature2 {
    #[input]
    pub problem: String,
    #[input]
    pub hint: i8,
    #[output]
    pub output: TestOutput,
}

#[tokio::test]
#[cfg_attr(miri, ignore)]
async fn test_chat_adapter_with_multiple_fields_and_output_schema() {
    let signature = TestSignature2::new();

    let lm = DummyLM::default();
    let adapter = ChatAdapter;

    let messages: Chat = adapter.format(
        &signature,
        Example::new(
            hashmap! {
                "problem".to_string() => "What is the capital of France?".to_string().into(),
                "hint".to_string() => "The capital of France is Paris.".to_string().into(),
            },
            vec!["problem".to_string(), "hint".to_string()],
            vec!["output".to_string()],
        ),
    );

    let json_value = messages.to_json();
    let json = json_value.as_array().unwrap();

    assert_eq!(messages.len(), 2);
    assert_eq!(json[0]["role"], "system");
    assert_eq!(json[1]["role"], "user");

    assert_eq!(
        json[0]["content"],
        "Your input fields are:\n1. `problem` (String)\n2. `hint` (i8)\n\nYour output fields are:\n1. `output` (TestOutput)\n\nAll interactions will be structured in the following way, with the appropriate values filled in.\n\n[[ ## problem ## ]]\nproblem\n\n[[ ## hint ## ]]\nhint\t# note: the value you produce must be a single i8 value\n\n[[ ## output ## ]]\noutput\t# note: the value you produce must adhere to the JSON schema: {\"reasoning\":{\"type\":\"string\"},\"rating\":{\"type\":\"integer\",\"format\":\"int8\",\"minimum\":-128,\"maximum\":127}}\n\n[[ ## completed ## ]]\n\nIn adhering to this structure, your objective is:\n\tGiven the fields `problem`, `hint`, produce the fields `output`.".to_string()
    );
    assert_eq!(
        json[1]["content"],
        "[[ ## problem ## ]]\nWhat is the capital of France?\n\n[[ ## hint ## ]]\nThe capital of France is Paris.\n\nRespond with the corresponding output fields, starting with the field `output` (must be formatted as valid Rust TestOutput), and then ending with the marker for `completed`."
    );

    let test_example = example! {
        "problem": "input" => "What is the capital of France?",
        "hint": "output" => "The capital of France is Paris.",
        "output": "output" => "{\"reasoning\": \"The capital of France is Paris.\", \"rating\": 5}"
    };

    let response = lm
        .call(
            test_example,
            Chat::new(vec![
                Message::system("You are a helpful assistant."),
                Message::user("Hello, world!"),
            ]),
            "[[ ## output ## ]]\n{\"reasoning\": \"The capital of France is Paris.\", \"rating\": 5}\n\n[[ ## completed ## ]]".to_string(),
        )
        .await
        .unwrap();
    let output = adapter.parse_response(&signature, response.output);

    assert_eq!(output.len(), 1);

    let parsed_output: serde_json::Value =
        serde_json::from_str("{\"reasoning\": \"The capital of France is Paris.\", \"rating\": 5}")
            .unwrap();
    assert_eq!(
        output.get("output").unwrap()["reasoning"],
        parsed_output["reasoning"]
    );
    assert_eq!(
        output.get("output").unwrap()["rating"],
        parsed_output["rating"]
    );
}

#[tokio::test]
#[cfg_attr(miri, ignore)]
async fn test_chat_adapter_with_demos() {
    let mut signature = sign! {
        (problem: String) -> answer: String
    };

    let adapter = ChatAdapter;

    // Create demo examples
    let demo1 = Example::new(
        hashmap! {
            "problem".to_string() => "What is 2 + 2?".to_string().into(),
            "answer".to_string() => "4".to_string().into(),
        },
        vec!["problem".to_string()],
        vec!["answer".to_string()],
    );

    let demo2 = Example::new(
        hashmap! {
            "problem".to_string() => "What is the largest planet?".to_string().into(),
            "answer".to_string() => "Jupiter".to_string().into(),
        },
        vec!["problem".to_string()],
        vec!["answer".to_string()],
    );

    signature.set_demos(vec![demo1, demo2]).unwrap();

    let current_input = Example::new(
        hashmap! {
            "problem".to_string() => "What is the capital of France?".to_string().into(),
        },
        vec!["problem".to_string()],
        vec!["answer".to_string()],
    );

    let messages: Chat = adapter.format(&signature, current_input);

    let json_value = messages.to_json();
    let json = json_value.as_array().unwrap();

    // Should have system message + 2 demo pairs (user + assistant) + current user message
    assert_eq!(messages.len(), 6);
    assert_eq!(json[0]["role"], "system");
    assert_eq!(json[1]["role"], "user");
    assert_eq!(json[2]["role"], "assistant");
    assert_eq!(json[3]["role"], "user");
    assert_eq!(json[4]["role"], "assistant");
    assert_eq!(json[5]["role"], "user");

    // Check demo 1 formatting
    assert!(
        json[1]["content"]
            .as_str()
            .unwrap()
            .contains("[[ ## problem ## ]]\nWhat is 2 + 2?")
    );
    assert!(
        json[2]["content"]
            .as_str()
            .unwrap()
            .contains("[[ ## answer ## ]]\n4")
    );
    assert!(
        json[2]["content"]
            .as_str()
            .unwrap()
            .contains("[[ ## completed ## ]]")
    );

    // Check demo 2 formatting
    assert!(
        json[3]["content"]
            .as_str()
            .unwrap()
            .contains("[[ ## problem ## ]]\nWhat is the largest planet?")
    );
    assert!(
        json[4]["content"]
            .as_str()
            .unwrap()
            .contains("[[ ## answer ## ]]\nJupiter")
    );
    assert!(
        json[4]["content"]
            .as_str()
            .unwrap()
            .contains("[[ ## completed ## ]]")
    );

    // Check current input formatting
    assert!(
        json[5]["content"]
            .as_str()
            .unwrap()
            .contains("[[ ## problem ## ]]\nWhat is the capital of France?")
    );
    assert!(
        json[5]["content"]
            .as_str()
            .unwrap()
            .contains("Respond with the corresponding output fields")
    );
}

#[tokio::test]
#[cfg_attr(miri, ignore)]
async fn test_chat_adapter_with_empty_demos() {
    let mut signature = sign! {
        (problem: String) -> answer: String
    };

    let adapter = ChatAdapter;

    let current_input = Example::new(
        hashmap! {
            "problem".to_string() => "What is the capital of France?".to_string().into(),
        },
        vec!["problem".to_string()],
        vec!["answer".to_string()],
    );
    signature.set_demos(vec![]).unwrap();

    let messages: Chat = adapter.format(&signature, current_input);

    let json_value = messages.to_json();
    let json = json_value.as_array().unwrap();

    // Should only have system message + current user message (no demos)
    assert_eq!(messages.len(), 2);
    assert_eq!(json[0]["role"], "system");
    assert_eq!(json[1]["role"], "user");

    // Check current input formatting
    assert!(
        json[1]["content"]
            .as_str()
            .unwrap()
            .contains("[[ ## problem ## ]]\nWhat is the capital of France?")
    );
}

#[tokio::test]
#[cfg_attr(miri, ignore)]
async fn test_chat_adapter_demo_format_multiple_fields() {
    let mut signature = TestSignature::new();

    let adapter = ChatAdapter;

    let demo = Example::new(
        hashmap! {
            "problem".to_string() => "What is 5 * 6?".to_string().into(),
            "hint".to_string() => "Think about multiplication".to_string().into(),
            "reasoning".to_string() => "5 multiplied by 6 equals 30".to_string().into(),
            "answer".to_string() => "30".to_string().into(),
        },
        vec!["problem".to_string(), "hint".to_string()],
        vec!["reasoning".to_string(), "answer".to_string()],
    );

    signature.set_demos(vec![demo]).unwrap();

    let current_input = Example::new(
        hashmap! {
            "problem".to_string() => "What is 3 + 7?".to_string().into(),
            "hint".to_string() => "Simple addition".to_string().into(),
        },
        vec!["problem".to_string(), "hint".to_string()],
        vec!["reasoning".to_string(), "answer".to_string()],
    );

    let messages: Chat = adapter.format(&signature, current_input);

    let json_value = messages.to_json();
    let json = json_value.as_array().unwrap();

    // Should have system + demo user + demo assistant + current user
    assert_eq!(messages.len(), 4);

    // Check demo user message contains both input fields
    assert!(
        json[1]["content"]
            .as_str()
            .unwrap()
            .contains("[[ ## problem ## ]]\nWhat is 5 * 6?")
    );
    assert!(
        json[1]["content"]
            .as_str()
            .unwrap()
            .contains("[[ ## hint ## ]]\nThink about multiplication")
    );

    // Check demo assistant message contains both output fields and completion marker
    assert!(
        json[2]["content"]
            .as_str()
            .unwrap()
            .contains("[[ ## reasoning ## ]]\n5 multiplied by 6 equals 30")
    );
    assert!(
        json[2]["content"]
            .as_str()
            .unwrap()
            .contains("[[ ## answer ## ]]\n30")
    );
    assert!(
        json[2]["content"]
            .as_str()
            .unwrap()
            .contains("[[ ## completed ## ]]")
    );
}

#[tokio::test]
#[cfg_attr(miri, ignore)]
async fn test_chat_adapter_with_cache_hit() {
    let dummy_lm = DummyLM::default();

    // Create test input example
    let input = example! {
        "question": "input" => "What is 2 + 2?",
    };

    // Create chat messages
    let chat = Chat::new(vec![
        Message::system("You are a helpful assistant."),
        Message::user("What is 2 + 2?"),
    ]);

    // First call - will cache the result
    let response1 = dummy_lm
        .call(
            input.clone(),
            chat.clone(),
            "[[ ## answer ## ]]\n4\n\n[[ ## completed ## ]]".to_string(),
        )
        .await
        .unwrap();

    // Second call with same input - should use cached result internally
    let response2 = dummy_lm
        .call(
            input.clone(),
            chat.clone(),
            "[[ ## answer ## ]]\n4\n\n[[ ## completed ## ]]".to_string(),
        )
        .await
        .unwrap();

    // Both responses should be identical
    assert_eq!(response1.output.content(), response2.output.content());
    assert_eq!(
        response1.output.content(),
        "[[ ## answer ## ]]\n4\n\n[[ ## completed ## ]]"
    );
}

#[tokio::test]
#[cfg_attr(miri, ignore)]
async fn test_chat_adapter_cache_miss_different_inputs() {
    // Create DummyLM with cache enabled

    let cache_handler = Arc::new(Mutex::new(Cache::new().await));
    let dummy_lm = DummyLM::builder()
        .cache_handler(cache_handler)
        .api_key("test_key".to_string())
        .build();

    // First input
    let input1 = example! {
        "question": "input" => "What is 2 + 2?",
    };

    // Second (different) input
    let input2 = example! {
        "question": "input" => "What is 3 + 3?",
    };

    let chat = Chat::new(vec![
        Message::system("You are a helpful assistant."),
        Message::user("Calculate the sum."),
    ]);

    // Call with first input
    let response1 = dummy_lm
        .call(
            input1.clone(),
            chat.clone(),
            "[[ ## answer ## ]]\n4\n\n[[ ## completed ## ]]".to_string(),
        )
        .await
        .unwrap();

    // Call with second input (different input, should not hit cache)
    let response2 = dummy_lm
        .call(
            input2.clone(),
            chat.clone(),
            "[[ ## answer ## ]]\n6\n\n[[ ## completed ## ]]".to_string(),
        )
        .await
        .unwrap();

    // Different inputs should produce different responses
    assert_eq!(
        response1.output.content(),
        "[[ ## answer ## ]]\n4\n\n[[ ## completed ## ]]"
    );
    assert_eq!(
        response2.output.content(),
        "[[ ## answer ## ]]\n6\n\n[[ ## completed ## ]]"
    );
    assert_ne!(response1.output.content(), response2.output.content());
}

#[tokio::test]
#[cfg_attr(miri, ignore)]
async fn test_chat_adapter_cache_disabled() {
    // Create DummyLM with cache disabled
    let dummy_lm = DummyLM::default();

    // Create test input
    let input = example! {
        "question": "input" => "What is 2 + 2?",
    };

    let chat = Chat::new(vec![
        Message::system("You are a helpful assistant."),
        Message::user("What is 2 + 2?"),
    ]);

    // Call without cache - should work normally
    let response = dummy_lm
        .call(
            input.clone(),
            chat.clone(),
            "[[ ## answer ## ]]\n4\n\n[[ ## completed ## ]]".to_string(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.output.content(),
        "[[ ## answer ## ]]\n4\n\n[[ ## completed ## ]]"
    );

    // Verify cache handler is None when cache is disabled
    assert!(dummy_lm.cache_handler.is_none());
}
