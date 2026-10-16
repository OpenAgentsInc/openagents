//! `openagents dm`: private messages between Nostr keys (NIP-17).
//!
//! `send` writes a kind-14 message from a Verse profile's key, seals it to
//! the reader with NIP-44 (kind 13), and gift-wraps the seal under a
//! one-time key (kind 1059), with both outer timestamps moved up to two
//! days earlier. It publishes one wrap to the reader and one to the sender,
//! so the sender's own inbox keeps what it sent. The relay sees only the
//! reader's public key on each wrap, and serves a wrap only to that reader
//! after NIP-42 authentication.
//!
//! `inbox` reads the wraps addressed to the profile, opens each one, and
//! prints the messages oldest first. A wrap that doesn't open, or holds
//! anything other than a chat message, is counted and skipped.

use std::collections::BTreeMap;
use std::str::FromStr;
use std::time::{Duration, Instant};

use nostr::domain::{Event, Tag};
use nostr::nip17;
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};

use crate::relay::{Client, DEFAULT_WAIT, identity_for, relay_url, unix_now};
use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents dm COMMAND [OPTIONS]
  send TO TEXT|- [--as PROFILE] [--relay URL]
        Send a private message to TO: an npub, a hex public key, or the name
        of one of your profiles. TEXT `-` reads it from stdin. Only TO and
        you can read it; the relay sees who it's for, not who sent it.
  inbox [--with WHO] [--since UNIX] [--as PROFILE] [--relay URL] [--wait SECONDS]
        Read your private messages, sent and received, oldest first.
        --with keeps one conversation.
--as names the Verse profile key (default: default; `openagents key show`
prints its public key). --relay defaults to wss://relay.openagents.com.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("send", Effect::Publishes),
    Declared::computer("inbox", Effect::ReadOnly),
];

/// How far `--since` reaches back, because each wrap's timestamp sits up
/// to two days before its message.
const WRAP_JITTER: u64 = nip17::RANDOMIZE_WINDOW_SECONDS;

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("dm", "a command is required", USAGE);
    };
    if command == "--help" || command == "-h" || command == "help" {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("dm", &message, USAGE),
    };
    let result = match command.as_str() {
        "send" => send(&args),
        "inbox" | "read" => inbox(&args),
        other => return output.usage("dm", &format!("unknown command `{other}`"), USAGE),
    };
    match result {
        Ok((value, render)) => {
            output.emit(&value, |_| render);
            0
        }
        Err(Failure::Usage(message)) => output.usage("dm", &message, USAGE),
        Err(Failure::Failed(message)) => output.fail("dm", &message),
    }
}

enum Failure {
    Usage(String),
    Failed(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

fn send(args: &Args) -> Result<(Value, String), Failure> {
    let [to, text] = args.positional() else {
        return Err(Failure::Usage("send takes TO and TEXT".into()));
    };
    let text = if text == "-" {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
            .map_err(|error| format!("cannot read stdin: {error}"))?;
        text.trim_end_matches('\n').to_owned()
    } else {
        text.clone()
    };
    if text.trim().is_empty() {
        return Err(Failure::Usage("the message is empty".into()));
    }
    let reader = recipient(to)?;
    let me = identity_for(args.option("as"))?;
    let author = me.signer.pubkey().to_owned();
    let mine = xonly(&author)?;
    let at = unix_now();
    let tags = vec![Tag::new(vec!["p".into(), reader.to_string()])];
    let rumor = nip17::chat_rumor(&author, at, &text, tags)
        .map_err(|error| format!("the message can't be built: {error}"))?;
    // One wrap for the reader, one for the sender's own record.
    let mut wraps = vec![wrap(&rumor, &me.secret, &reader, at)?];
    if reader != mine {
        wraps.push(wrap(&rumor, &me.secret, &mine, at)?);
    }
    let url = relay_url(args.option("relay"));
    let mut client = Client::connect(&url, me.signer);
    let mut refused = None;
    for event in wraps {
        match client.publish(event, DEFAULT_WAIT) {
            Ok(published) if published.accepted => {}
            Ok(published) => {
                refused = Some(published.message);
                break;
            }
            Err(error) => {
                refused = Some(error);
                break;
            }
        }
    }
    client.close();
    if let Some(reason) = refused {
        return Err(Failure::Failed(format!(
            "{url} refused the message: {reason}"
        )));
    }
    let to_npub = npub(&reader.to_string());
    Ok((
        json!({
            "id": rumor.id,
            "from": npub(&author),
            "to": to_npub,
            "relay": url,
            "at": at,
        }),
        format!("Sent to {to_npub} through {url}."),
    ))
}

fn inbox(args: &Args) -> Result<(Value, String), Failure> {
    let me = identity_for(args.option("as"))?;
    let mine = me.signer.pubkey().to_owned();
    let with = args.option("with").map(recipient).transpose()?;
    let wait = Duration::from_secs(
        args.number("wait", DEFAULT_WAIT.as_secs())
            .map_err(Failure::Usage)?,
    );
    let mut filter = json!({"kinds": [1059], "#p": [mine]});
    if let Some(since) = args.option("since") {
        let since: u64 = since
            .parse()
            .map_err(|_| Failure::Usage("--since takes Unix seconds".into()))?;
        filter["since"] = json!(since.saturating_sub(WRAP_JITTER));
    }
    let url = relay_url(args.option("relay"));
    let mut client = Client::connect(&url, me.signer);
    let mut wraps: Vec<Event> = Vec::new();
    let mut read = |client: &mut Client| {
        client.subscribe(vec![filter.clone()], false, wait, |event| {
            wraps.push(event.clone());
        })
    };
    // The relay serves gift wraps only to the authenticated reader.
    if let Err(error) = read(&mut client) {
        if !error.contains("auth-required") || !client.authenticate(Instant::now() + wait) {
            client.close();
            return Err(Failure::Failed(error));
        }
        read(&mut client).map_err(Failure::Failed)?;
    }
    client.close();
    let (messages, unopened) = open_all(&wraps, &me.secret, &mine, with.as_ref());
    let rendered = render(&messages, &mine);
    Ok((
        json!({
            "me": npub(&mine),
            "relay": url,
            "messages": messages,
            "unopened": unopened,
        }),
        if rendered.is_empty() {
            format!("No private messages for {} on {url}.", npub(&mine))
        } else {
            rendered
        },
    ))
}

/// Opens every wrap for `reader`, keeps chat messages (one per message id,
/// since a wrap can arrive twice), and returns them oldest first with the
/// count of wraps that didn't open.
fn open_all(
    wraps: &[Event],
    reader: &SecretKey,
    mine: &str,
    with: Option<&XOnlyPublicKey>,
) -> (Vec<Value>, usize) {
    let mut messages = BTreeMap::new();
    let mut unopened = 0;
    for event in wraps {
        let Ok(message) =
            nip17::open_direct_message(event, reader).and_then(|rumor| nip17::chat_message(&rumor))
        else {
            unopened += 1;
            continue;
        };
        let peers: Vec<&String> = if message.pubkey == mine {
            message.receivers.iter().collect()
        } else {
            vec![&message.pubkey]
        };
        if let Some(with) = with {
            let with = with.to_string();
            if !peers.iter().any(|peer| **peer == with) {
                continue;
            }
        }
        messages.insert(
            (message.created_at, message.id.clone()),
            json!({
                "id": message.id,
                "at": message.created_at,
                "from": npub(&message.pubkey),
                "to": message.receivers.iter().map(|key| npub(key)).collect::<Vec<_>>(),
                "sent": message.pubkey == mine,
                "text": message.content,
            }),
        );
    }
    (messages.into_values().collect(), unopened)
}

fn render(messages: &[Value], mine: &str) -> String {
    let me = npub(mine);
    messages
        .iter()
        .map(|message| {
            let when = crate::relay::when(message["at"].as_u64().unwrap_or(0));
            let text = message["text"].as_str().unwrap_or("");
            if message["sent"].as_bool().unwrap_or(false) {
                let to = message["to"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|key| *key != me)
                    .map(short)
                    .collect::<Vec<_>>()
                    .join(", ");
                let to = if to.is_empty() {
                    "yourself".to_owned()
                } else {
                    to
                };
                format!("{when}  you → {to}: {text}")
            } else {
                let from = short(message["from"].as_str().unwrap_or(""));
                format!("{when}  {from}: {text}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn short(npub: &str) -> String {
    if npub.len() > 16 {
        format!("{}…{}", &npub[..10], &npub[npub.len() - 4..])
    } else {
        npub.to_owned()
    }
}

fn wrap(
    rumor: &nip17::Rumor,
    author: &SecretKey,
    reader: &XOnlyPublicKey,
    at: u64,
) -> Result<Event, Failure> {
    let failed = |error: nostr::domain::DomainError| {
        Failure::Failed(format!("the message can't be sealed: {error}"))
    };
    let seal = nip17::seal(
        rumor,
        author,
        reader,
        nip17::hidden_timestamp(at, jitter()?).map_err(failed)?,
        random_bytes()?,
        None,
    )
    .map_err(failed)?;
    let wrapper = SecretKey::new(&mut secp256k1::rand::rng());
    nip17::gift_wrap(
        &seal,
        &wrapper,
        reader,
        nip17::hidden_timestamp(at, jitter()?).map_err(failed)?,
        random_bytes()?,
        None,
    )
    .map_err(failed)
}

fn random_bytes() -> Result<[u8; 32], Failure> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| Failure::Failed("no randomness available".into()))?;
    Ok(bytes)
}

/// Seconds to move an outer timestamp earlier, up to two days.
fn jitter() -> Result<u64, Failure> {
    let bytes = random_bytes()?;
    let value = u64::from_le_bytes(bytes[..8].try_into().unwrap_or([0; 8]));
    Ok(value % (WRAP_JITTER + 1))
}

/// An npub, a hex public key, or the name of a local profile.
fn recipient(text: &str) -> Result<XOnlyPublicKey, Failure> {
    let text = text.trim();
    if text.starts_with("npub1") {
        let bytes = nostr::nip19::decode_npub(text)
            .map_err(|_| Failure::Usage(format!("{text} isn't a valid npub")))?;
        return XOnlyPublicKey::from_byte_array(bytes)
            .map_err(|_| Failure::Usage(format!("{text} isn't a valid public key")));
    }
    if text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit()) {
        return xonly(&text.to_ascii_lowercase());
    }
    let path = verse_net::identity::home().join(format!("{text}.key"));
    if path.is_file() {
        let identity = identity_for(Some(text))?;
        return xonly(identity.signer.pubkey());
    }
    Err(Failure::Usage(format!(
        "{text} isn't an npub, a hex public key, or one of your profiles (`openagents key list`)"
    )))
}

fn xonly(hex: &str) -> Result<XOnlyPublicKey, Failure> {
    XOnlyPublicKey::from_str(hex).map_err(|_| Failure::Usage(format!("{hex} isn't a public key")))
}

fn npub(hex: &str) -> String {
    let mut bytes = [0u8; 32];
    if hex.len() == 64
        && hex.is_char_boundary(64)
        && (0..32).all(|i| {
            u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map(|byte| bytes[i] = byte)
                .is_ok()
        })
    {
        nostr::nip19::encode_npub(&bytes)
    } else {
        hex.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::Secp256k1;

    fn key(byte: u8) -> SecretKey {
        SecretKey::from_byte_array([byte; 32]).unwrap()
    }

    fn public(secret: &SecretKey) -> XOnlyPublicKey {
        secret.x_only_public_key(&Secp256k1::new()).0
    }

    #[test]
    fn a_message_opens_for_the_reader_and_the_sender_only() {
        let (alice, bob, eve) = (key(1), key(2), key(3));
        let author = public(&alice).to_string();
        let tags = vec![Tag::new(vec!["p".into(), public(&bob).to_string()])];
        let rumor = nip17::chat_rumor(&author, 1_791_900_000, "hello bob", tags).unwrap();
        let to_bob = wrap(&rumor, &alice, &public(&bob), 1_791_900_000)
            .ok()
            .unwrap();
        let to_alice = wrap(&rumor, &alice, &public(&alice), 1_791_900_000)
            .ok()
            .unwrap();
        assert_ne!(to_bob.pubkey, author, "the wrap hides the author");

        let bob_hex = public(&bob).to_string();
        let (read, unopened) = open_all(&[to_bob.clone()], &bob, &bob_hex, None);
        assert_eq!((read.len(), unopened), (1, 0));
        assert_eq!(read[0]["text"], "hello bob");
        assert_eq!(read[0]["sent"], false);

        // Alice's own copy reads as sent, and a repeat delivery counts once.
        let alice_hex = public(&alice).to_string();
        let (sent, _) = open_all(&[to_alice.clone(), to_alice], &alice, &alice_hex, None);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["sent"], true);
        assert!(render(&sent, &alice_hex).contains("you → npub1"));

        // Nobody else opens Bob's wrap.
        let eve_hex = public(&eve).to_string();
        assert_eq!(open_all(&[to_bob], &eve, &eve_hex, None), (vec![], 1));
    }

    #[test]
    fn with_keeps_one_conversation() {
        let (alice, bob, carol) = (key(1), key(2), key(4));
        let bob_hex = public(&bob).to_string();
        let mut wraps = Vec::new();
        for (from, text) in [(&alice, "from alice"), (&carol, "from carol")] {
            let tags = vec![Tag::new(vec!["p".into(), bob_hex.clone()])];
            let rumor =
                nip17::chat_rumor(&public(from).to_string(), 1_791_900_000, text, tags).unwrap();
            wraps.push(
                wrap(&rumor, from, &public(&bob), 1_791_900_000)
                    .ok()
                    .unwrap(),
            );
        }
        let (only, _) = open_all(&wraps, &bob, &bob_hex, Some(&public(&carol)));
        assert_eq!(only.len(), 1);
        assert_eq!(only[0]["text"], "from carol");
    }

    #[test]
    fn recipients_take_npub_and_hex() {
        let hex = public(&key(5)).to_string();
        let npub = npub(&hex);
        assert!(npub.starts_with("npub1"));
        assert_eq!(recipient(&npub).ok().unwrap().to_string(), hex);
        assert_eq!(
            recipient(&hex.to_uppercase()).ok().unwrap().to_string(),
            hex
        );
        assert!(recipient("npub1nope").is_err());
    }
}
