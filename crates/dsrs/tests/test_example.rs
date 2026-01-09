use dsrs::data::example::Example;
use dsrs::data::serialize::{load_jsonl, save_examples_as_jsonl};
use dsrs::{example, hashmap};
use rstest::*;

#[rstest]
fn test_initialization() {
    let data = hashmap! {
        "input1".to_string() => "value1".to_string().into(),
        "input2".to_string() => "value2".to_string().into(),
        "output1".to_string() => "value3".to_string().into(),
    };

    let input_keys = vec!["input1".to_string(), "input2".to_string()];
    let output_keys = vec!["output1".to_string()];
    let example = Example::new(data.clone(), input_keys.clone(), output_keys.clone());

    assert_eq!(example.data, data);
    assert_eq!(example.input_keys, input_keys);
    assert_eq!(example.output_keys, output_keys);
}

#[rstest]
fn test_get() {
    let data = hashmap! {
        "input1".to_string() => "value1".to_string().into(),
        "input2".to_string() => "value2".to_string().into(),
        "output1".to_string() => "value3".to_string().into(),
    };
    let input_keys = vec!["input1".to_string(), "input2".to_string()];
    let output_keys = vec!["output1".to_string()];
    let example = Example::new(data, input_keys, output_keys);

    assert_eq!(example.get("input1", None), "value1");
    assert_eq!(example.get("input2", None), "value2");
    assert_eq!(example.get("output1", None), "value3");
    assert_eq!(example.get("input3", None), "");
    assert_eq!(example.get("input3", Some("default")), "default");
}

#[rstest]
fn test_keys() {
    let data = hashmap! {
        "input1".to_string() => "value1".to_string().into(),
        "input2".to_string() => "value2".to_string().into(),
        "output1".to_string() => "value3".to_string().into(),
    };
    let input_keys = vec!["input1".to_string(), "input2".to_string()];
    let output_keys = vec!["output1".to_string()];
    let example = Example::new(data, input_keys, output_keys);

    let mut keys = example.keys();
    keys.sort();
    assert_eq!(keys, vec!["input1", "input2", "output1"]);
}

#[rstest]
fn test_values() {
    let data = hashmap! {
        "input1".to_string() => "value1".to_string().into(),
        "input2".to_string() => "value2".to_string().into(),
        "output1".to_string() => "value3".to_string().into(),
    };
    let input_keys = vec!["input1".to_string(), "input2".to_string()];
    let output_keys = vec!["output1".to_string()];
    let example = Example::new(data, input_keys, output_keys);

    let mut values = example.values();
    values.sort_by_key(|v| v.to_string());
    assert_eq!(values, vec!["value1", "value2", "value3"]);
}

#[rstest]
fn test_set_input_keys() {
    let data = hashmap! {
        "input1".to_string() => "value1".to_string().into(),
        "input2".to_string() => "value2".to_string().into(),
        "output1".to_string() => "value3".to_string().into(),
    };

    let mut example = Example::new(
        data,
        vec!["input2".to_string()],
        vec!["output1".to_string()],
    );
    example.set_input_keys(vec!["input1".to_string()]);

    assert_eq!(example.input_keys, vec!["input1".to_string()]);

    // output_keys should now contain all keys not in input_keys
    let mut output_keys = example.output_keys.clone();
    output_keys.sort();
    assert_eq!(output_keys, vec!["input2", "output1"]);
}

#[rstest]
fn test_without() {
    let data = hashmap! {
        "input1".to_string() => "value1".to_string().into(),
        "input2".to_string() => "value2".to_string().into(),
        "output1".to_string() => "value3".to_string().into(),
    };
    let input_keys = vec!["input1".to_string(), "input2".to_string()];
    let output_keys = vec!["output1".to_string()];
    let example = Example::new(data, input_keys, output_keys);

    let without_input1 = example.without(vec!["input1".to_string()]);
    assert_eq!(without_input1.input_keys, vec!["input2".to_string()]);
    assert_eq!(without_input1.output_keys, vec!["output1".to_string()]);
}

#[rstest]
#[cfg_attr(miri, ignore = "MIRI has issues with rayon's parallel iterators")]
fn test_serialize() {
    let examples = vec![
        Example::new(
            hashmap! {
                "input1".to_string() => "value1".to_string().into(),
            },
            vec!["input1".to_string()],
            vec!["output1".to_string()],
        ),
        Example::new(
            hashmap! {
                "input1".to_string() => "value2".to_string().into(),
            },
            vec!["input1".to_string()],
            vec!["output1".to_string()],
        ),
    ];
    save_examples_as_jsonl("/tmp/examples.jsonl", examples);
    let examples = load_jsonl(
        "/tmp/examples.jsonl",
        vec!["input1".to_string()],
        vec!["output1".to_string()],
    );
    assert_eq!(examples.len(), 2);
    assert_eq!(
        examples[0].data,
        hashmap! {
            "input1".to_string() => "value1".to_string().into(),
        }
    );
    assert_eq!(
        examples[1].data,
        hashmap! {
            "input1".to_string() => "value2".to_string().into(),
        }
    );
    assert_eq!(examples[0].input_keys, vec!["input1".to_string()]);
    assert_eq!(examples[1].input_keys, vec!["input1".to_string()]);
    assert_eq!(examples[0].output_keys, vec!["output1".to_string()]);
    assert_eq!(examples[1].output_keys, vec!["output1".to_string()]);
}

#[rstest]
fn test_example_macro() {
    let example = example! {
        "question": "input" => "What is the capital of France?",
        "answer": "output" => "Paris"
    };
    assert_eq!(
        example.data,
        hashmap! {
            "question".to_string() => "What is the capital of France?".to_string().into(),
            "answer".to_string() => "Paris".to_string().into(),
        }
    );

    let example = example! {
        "question": "input" => "What is the capital of France?",
        "answer": "output" => "Paris"
    };
    assert_eq!(example.input_keys, vec!["question".to_string()]);
    assert_eq!(example.output_keys, vec!["answer".to_string()]);
    assert_eq!(
        example.data,
        hashmap! {
            "question".to_string() => "What is the capital of France?".to_string().into(),
            "answer".to_string() => "Paris".to_string().into(),
        }
    );
}
