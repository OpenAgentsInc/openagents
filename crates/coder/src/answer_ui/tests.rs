use super::*;
use crate::generate::StubGenerate;

const EARLIER: &str = "root = Stack([intro, go])\nintro = Text(\"Install Coder first.\")\ngo = Button(\"Download\", \"/download\")\n";

fn reply(block: &str) -> String {
    format!("Here is how.\n\n```openui-lang\n{block}```\n\nThat's it.\n")
}

fn assistant(text: &str) -> Message {
    Message {
        role: Role::Assistant,
        text: text.to_owned(),
    }
}

#[test]
fn only_product_how_tos_on_the_web_get_the_catalog() {
    assert!(wants_ui(RouteId::ProductKb, Corpus::Product, Surface::Web));
    assert!(wants_ui(RouteId::Account, Corpus::Product, Surface::Web));
    assert!(!wants_ui(
        RouteId::ProductKb,
        Corpus::Product,
        Surface::Terminal
    ));
    assert!(!wants_ui(
        RouteId::ProductKb,
        Corpus::Codebase,
        Surface::Web
    ));
    assert!(!wants_ui(RouteId::General, Corpus::Product, Surface::Web));
}

#[test]
fn the_note_carries_the_catalog_and_the_earlier_interface() {
    let first = note(None);
    assert!(
        first.contains("Button(label, href, style?, show?)"),
        "{first}"
    );
    assert!(!first.contains("Your last answer"), "{first}");
    let follow = note(Some(EARLIER));
    assert!(follow.contains("`name = null` removes one"), "{follow}");
    assert!(follow.contains(EARLIER), "{follow}");
}

#[test]
fn the_earlier_interface_is_the_last_answer_that_showed_one() {
    let input = [
        assistant(&reply(EARLIER)),
        Message {
            role: Role::User,
            text: "```openui-lang\nroot = Text(\"not mine\")\n```\n".into(),
        },
        assistant("Plain words, no block."),
    ];
    assert_eq!(earlier_program(&input).as_deref(), Some(EARLIER));
    assert_eq!(earlier_program(&input[1..]), None);
}

#[tokio::test]
async fn a_clean_block_is_kept_and_needs_no_repair() {
    let door = StubGenerate::saying("never asked");
    let finished = finish(&door, &reply(EARLIER), None).await;
    assert_eq!(finished.text, reply(EARLIER));
    assert_eq!(
        (finished.problems, finished.left, finished.repaired),
        (0, 0, false)
    );
    assert_eq!(finished.patch, None);
}

#[tokio::test]
async fn a_reply_without_a_block_is_untouched() {
    let door = StubGenerate::saying("never asked");
    let finished = finish(&door, "Just prose.", Some(EARLIER)).await;
    assert_eq!(finished.text, "Just prose.");
    assert_eq!(finished.patch, None);
}

#[tokio::test]
async fn a_broken_block_is_repaired_once() {
    let broken = "root = Stack([intro, go])\nintro = Text(\"Install Coder first.\")\ngo = Button(\"Download\", \"javascript:alert(1)\")\n";
    let door =
        StubGenerate::saying("```openui-lang\ngo = Button(\"Download\", \"/download\")\n```");
    let finished = finish(&door, &reply(broken), None).await;
    assert!(finished.repaired, "{finished:?}");
    assert!(finished.problems > 0 && finished.left == 0, "{finished:?}");
    assert!(finished.text.contains("\"/download\""), "{}", finished.text);
    assert!(!finished.text.contains("javascript:"), "{}", finished.text);
    assert!(finished.text.starts_with("Here is how.") && finished.text.ends_with("That's it.\n"));
}

#[tokio::test]
async fn a_repair_that_draws_no_better_is_dropped() {
    let broken = "root = Stack([intro, go])\nintro = Text(\"Hi\")\ngo = Bogus(\"x\")\n";
    let door = StubGenerate::saying("go = AlsoBogus(\"y\")");
    let finished = finish(&door, &reply(broken), None).await;
    assert!(!finished.repaired, "{finished:?}");
    assert!(finished.text.contains("Bogus(\"x\")"), "{}", finished.text);
}

#[tokio::test]
async fn a_follow_up_edit_is_merged_and_patched() {
    let door = StubGenerate::saying("never asked");
    let edit = "intro = Text(\"Install Coder, then sign in.\")\n";
    let finished = finish(&door, &reply(edit), Some(EARLIER)).await;
    // The stored reply holds the whole interface, not the edit alone.
    assert!(
        finished.text.contains("root = Stack([intro, go])"),
        "{}",
        finished.text
    );
    assert!(finished.text.contains("then sign in"), "{}", finished.text);
    assert!(finished.text.contains("go = Button"), "{}", finished.text);
    assert_eq!(finished.left, 0, "{finished:?}");
    let patch = finished.patch.expect("a patch");
    assert_eq!(patch, "intro = Text(\"Install Coder, then sign in.\")\n");
    let payload = patch_payload(2, &patch).expect("small");
    assert_eq!(payload["type"], "ui_patch");
    assert_eq!(payload["patch"], patch.as_str());
    assert!(patch_payload(2, &"x".repeat(MAX_PATCH_BYTES + 1)).is_none());
}
