use gpt_oss::{HarmonyRenderer, HarmonyRole, HarmonyTurn};
use openai_harmony::chat::Message;

#[test]
fn test_harmony_render_prompt_basic() {
    let renderer = HarmonyRenderer::gpt_oss().expect("Encoding should load");
    let turns = vec![HarmonyTurn::new(HarmonyRole::User, "Hello there")];

    let prompt = renderer
        .render_prompt(&turns, &[])
        .expect("Should render prompt");

    assert!(prompt.contains("<|start|>system"));
    assert!(prompt.contains("Hello there"));
}

#[test]
fn test_harmony_extract_assistant_text() {
    let renderer = HarmonyRenderer::gpt_oss().expect("Encoding should load");
    let message = Message::from_role_and_content(HarmonyRole::Assistant, "Hi!");

    let tokens = renderer.encoding().render(&message, None).unwrap();
    let completion = renderer
        .encoding()
        .tokenizer()
        .decode_utf8(&tokens)
        .unwrap();

    let text = renderer
        .extract_assistant_text(&completion)
        .expect("Should parse assistant content");

    assert_eq!(text, "Hi!");
}
