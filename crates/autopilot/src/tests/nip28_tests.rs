use crate::app::nip28::{Nip28Message, Nip28State};
use crate::commands::{parse_command, Command};

#[test]
fn parse_nip28_commands() {
    assert_eq!(parse_command("/nip28"), Some(Command::Nip28));
    assert_eq!(
        parse_command("/nip28 connect wss://relay.example"),
        Some(Command::Nip28Connect("wss://relay.example".to_string()))
    );
    assert_eq!(
        parse_command("/nip28 channel openagents"),
        Some(Command::Nip28Channel("openagents".to_string()))
    );
    assert_eq!(
        parse_command("/nip28 send hello world"),
        Some(Command::Nip28Send("hello world".to_string()))
    );
    assert_eq!(parse_command("/nip28 refresh"), Some(Command::Nip28Refresh));
}

#[test]
fn nip28_message_limit_is_capped() {
    let mut state = Nip28State::new();
    for idx in 0..205 {
        state.push_message(Nip28Message {
            _id: format!("id-{}", idx),
            pubkey: "pubkey".to_string(),
            content: "hello".to_string(),
            created_at: 0,
        });
    }
    assert_eq!(state.messages.len(), 200);
}

#[test]
fn nip28_input_editing() {
    let mut state = Nip28State::new();
    state.insert_text("hi");
    assert_eq!(state.input, "hi");
    state.move_cursor_left();
    state.insert_text("o");
    assert_eq!(state.input, "hoi");
    state.backspace();
    assert_eq!(state.input, "hi");
    state.move_cursor_end();
    state.insert_text("!");
    assert_eq!(state.input, "hi!");
    state.move_cursor_left();
    state.delete();
    assert_eq!(state.input, "hi");
}
