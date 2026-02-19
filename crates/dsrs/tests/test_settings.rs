use dsrs::{ChatAdapter, LM, configure, get_lm};

#[tokio::test]
#[cfg_attr(miri, ignore)]
async fn test_settings() {
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "test");
    }
    configure(
        LM::builder()
            .model("openai:gpt-4o-mini".to_string())
            .build()
            .await
            .unwrap(),
        ChatAdapter {},
    );

    let lm = get_lm();
    assert_eq!(lm.model, "openai:gpt-4o-mini");

    configure(
        LM::builder()
            .model("openai:gpt-4o".to_string())
            .build()
            .await
            .unwrap(),
        ChatAdapter {},
    );

    let lm = get_lm();

    assert_eq!(lm.model, "openai:gpt-4o");
}
