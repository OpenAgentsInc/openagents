//! NIP-A0 voice messages.
//!
//! Kind `1222` is a root message. Kind `1244` is a reply and uses the
//! NIP-22 root and parent tags. `content` is an `http://` or `https://`
//! URL of the audio file. An optional `imeta` tag carries a `waveform`
//! of whole-number amplitudes and a `duration` in seconds.
//!
//! The relay does not download the audio, so it does not check the
//! codec or the 60-second guidance. A duration over 60 seconds is kept.
//! The suggestion of fewer than 100 waveform amplitudes is not enforced.
//! NIP-A0 is a draft, so these kinds stay off the NIP-11 list. A voice
//! message is a regular event, so a newer one does not replace an older
//! one.

use super::comment::Comment;
use super::{DomainError, Event};

const ROOT_KIND: u16 = 1_222;
const REPLY_KIND: u16 = 1_244;

/// A kind `1222` root or a kind `1244` reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceMessage {
    pub audio_url: String,
    pub waveform: Option<Vec<u32>>,
    pub duration_seconds: Option<u32>,
    /// Present for a kind `1244` reply.
    pub thread: Option<Comment>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn audio_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn whole_number(token: &str) -> Result<u32, DomainError> {
    if token.is_empty()
        || (token.len() > 1 && token.starts_with('0'))
        || !token.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid("a waveform value is a whole number"));
    }
    token
        .parse()
        .map_err(|_| invalid("a waveform value is a whole number"))
}

fn waveform(value: &str) -> Result<Vec<u32>, DomainError> {
    let mut samples = Vec::new();
    for token in value.split(' ') {
        samples.push(whole_number(token)?);
    }
    Ok(samples)
}

fn duration(value: &str) -> Result<u32, DomainError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid("a duration is a whole number of seconds"));
    }
    value
        .parse()
        .map_err(|_| invalid("a duration is a whole number of seconds"))
}

fn preview(event: &Event, audio: &str) -> Result<(Option<Vec<u32>>, Option<u32>), DomainError> {
    let tags: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("imeta"))
        .collect();
    if tags.len() > 1 {
        return Err(invalid("a voice message has one imeta tag"));
    }
    let Some(tag) = tags.first() else {
        return Ok((None, None));
    };
    let mut url = None;
    let mut samples = None;
    let mut seconds = None;
    let mut extras = 0u32;
    for part in tag.as_slice().iter().skip(1) {
        let Some((key, value)) = part.split_once(' ') else {
            return Err(invalid("an imeta field is a key and a value"));
        };
        if key.is_empty() || value.is_empty() {
            return Err(invalid("an imeta field is a key and a value"));
        }
        match key {
            "url" => {
                if url.is_some() {
                    return Err(invalid("an imeta tag repeats url"));
                }
                url = Some(value);
            }
            "waveform" => {
                if samples.is_some() {
                    return Err(invalid("an imeta tag repeats waveform"));
                }
                samples = Some(waveform(value)?);
            }
            "duration" => {
                if seconds.is_some() {
                    return Err(invalid("an imeta tag repeats duration"));
                }
                seconds = Some(duration(value)?);
            }
            _ => extras += 1,
        }
    }
    let Some(url) = url else {
        return Err(invalid("an imeta tag requires a url"));
    };
    if url != audio {
        return Err(invalid("an imeta url matches the voice message"));
    }
    if samples.is_none() && seconds.is_none() && extras == 0 {
        return Err(invalid("an imeta tag requires another field"));
    }
    Ok((samples, seconds))
}

fn message(event: &Event, thread: Option<Comment>) -> Result<VoiceMessage, DomainError> {
    if !audio_url(&event.content) {
        return Err(invalid(
            "voice message content is an http:// or https:// audio URL",
        ));
    }
    let (waveform, duration_seconds) = preview(event, &event.content)?;
    Ok(VoiceMessage {
        audio_url: event.content.clone(),
        waveform,
        duration_seconds,
        thread,
    })
}

/// Read a kind `1222` root voice message.
pub fn open_voice_message(event: &Event) -> Result<VoiceMessage, DomainError> {
    if event.kind != ROOT_KIND {
        return Err(invalid("a voice message has kind 1222"));
    }
    message(event, None)
}

/// Read a kind `1244` voice reply.
///
/// The reply must carry NIP-22 root and parent tags. `content` is the
/// audio URL, the same rule as a root message.
pub fn open_voice_reply(event: &Event) -> Result<VoiceMessage, DomainError> {
    if event.kind != REPLY_KIND {
        return Err(invalid("a voice reply has kind 1244"));
    }
    let thread = super::comment::comment_scopes(event)?;
    message(event, Some(thread))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CommentScope, DomainError, EventClass, RelaySigner, Tag, compare_replacement, is_top_level,
    };

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    #[test]
    fn a_voice_message_keeps_its_audio_url_and_a_reply_threads_to_it() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/A0.md"
        ))
        .unwrap();
        assert!(text.contains("1222"));
        assert!(text.contains("1244"));
        assert!(text.contains("imeta"));
        assert!(text.contains("waveform"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "A0.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "A0.md")
        );

        let url = "https://cdn.example/voice/hello.m4a";
        let author = signer("a0");
        let root = author.sign(
            1_700_000_000,
            ROOT_KIND,
            vec![
                Tag::new(vec!["t".into(), "voice".into()]),
                Tag::new(vec![
                    "imeta".into(),
                    format!("url {url}"),
                    "waveform 0 7 35 8 100".into(),
                    "duration 8".into(),
                ]),
            ],
            url.into(),
        );
        root.validate_structure().unwrap();
        assert_eq!(root.class(), EventClass::Regular);
        let message = open_voice_message(&root).unwrap();
        assert_eq!(message.audio_url, url);
        assert_eq!(message.waveform.as_deref(), Some(&[0, 7, 35, 8, 100][..]));
        assert_eq!(message.duration_seconds, Some(8));
        assert!(message.thread.is_none());

        let long = author.sign(
            1_700_000_050,
            ROOT_KIND,
            vec![Tag::new(vec![
                "imeta".into(),
                format!("url {url}"),
                "duration 90".into(),
            ])],
            url.into(),
        );
        long.validate_structure().unwrap();
        assert_eq!(
            open_voice_message(&long).unwrap().duration_seconds,
            Some(90)
        );

        let again = author.sign(1_700_000_100, ROOT_KIND, Vec::new(), url.into());
        again.validate_structure().unwrap();
        assert!(matches!(
            compare_replacement(&root, &again),
            Err(DomainError::NotReplaceable)
        ));

        let prose = author.sign(1_700_000_200, ROOT_KIND, Vec::new(), "not a url".into());
        assert!(prose.validate_structure().is_err());
        let fractional = author.sign(
            1_700_000_300,
            ROOT_KIND,
            vec![Tag::new(vec![
                "imeta".into(),
                format!("url {url}"),
                "waveform 1 2.5".into(),
            ])],
            url.into(),
        );
        assert!(fractional.validate_structure().is_err());

        let reply = signer("a1").sign(
            1_700_000_400,
            REPLY_KIND,
            vec![
                Tag::new(vec![
                    "E".into(),
                    root.id.clone(),
                    "wss://relay.example".into(),
                    root.pubkey.clone(),
                ]),
                Tag::new(vec!["K".into(), "1222".into()]),
                Tag::new(vec![
                    "P".into(),
                    root.pubkey.clone(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec![
                    "e".into(),
                    root.id.clone(),
                    "wss://relay.example".into(),
                    root.pubkey.clone(),
                ]),
                Tag::new(vec!["k".into(), "1222".into()]),
                Tag::new(vec![
                    "p".into(),
                    root.pubkey.clone(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec![
                    "imeta".into(),
                    format!("url {url}"),
                    "m audio/mp4".into(),
                    "duration 8".into(),
                ]),
            ],
            url.into(),
        );
        reply.validate_structure().unwrap();
        assert_eq!(reply.class(), EventClass::Regular);
        let spoken = open_voice_reply(&reply).unwrap();
        assert_eq!(spoken.audio_url, url);
        assert_eq!(spoken.duration_seconds, Some(8));
        let thread = spoken.thread.as_ref().unwrap();
        assert!(is_top_level(thread));
        match &thread.parent {
            CommentScope::Event { id, author, .. } => {
                assert_eq!(id, &root.id);
                assert_eq!(author, &root.pubkey);
            }
            other => panic!("expected an event parent, got {other:?}"),
        }
        assert!(matches!(
            compare_replacement(&reply, &reply),
            Err(DomainError::NotReplaceable)
        ));

        let unthreaded = signer("a1").sign(1_700_000_500, REPLY_KIND, Vec::new(), url.into());
        assert!(unthreaded.validate_structure().is_err());
    }
}
