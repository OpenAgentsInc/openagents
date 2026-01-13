use crate::app::nip90::{Nip90Message, Nip90MessageKind, Nip90State};
use crate::commands::{Command, parse_command};

#[test]
fn parse_nip90_commands() {
    assert_eq!(parse_command("/nip90"), Some(Command::Nip90));
    assert_eq!(
        parse_command("/nip90 connect wss://relay.example"),
        Some(Command::Nip90Connect("wss://relay.example".to_string()))
    );
    assert_eq!(parse_command("/nip90 refresh"), Some(Command::Nip90Refresh));
}

#[test]
fn nip90_message_limit_is_capped() {
    let mut state = Nip90State::new();
    for idx in 0..205 {
        state.push_message(Nip90Message {
            kind: 5050,
            message_kind: Nip90MessageKind::Request,
            pubkey: "pubkey".to_string(),
            created_at: 0,
            summary: format!("job {}", idx),
        });
    }
    assert_eq!(state.messages.len(), 200);
}
