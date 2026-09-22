//! NIP-71 video events.
//!
//! Kinds `21` and `22` are regular video events — `22` for short
//! portrait clips — and kinds `34235`/`34236` are their addressable
//! counterparts carrying a `d` identifier. Every video event needs a
//! `title` tag and at least one `imeta` describing a video or audio
//! stream; `duration`, `bitrate`, `waveform`, `image`, `fallback`,
//! and `service` fields follow NIP-92 and NIP-94, and `text-track`
//! tags link caption files. NIP-71 is a draft, so the kinds are not
//! added to the NIP-11 list.

use super::imeta::{Imeta, open_imetas};
use super::{DomainError, Event};

/// The regular video kinds: `21` normal, `22` short vertical.
pub const VIDEO_KINDS: &[u16] = &[21, 22];
/// The addressable video kinds: `34235` normal, `34236` short.
pub const ADDRESSABLE_VIDEO_KINDS: &[u16] = &[34_235, 34_236];

/// A validated video event.
#[derive(Clone, Debug)]
pub struct Video {
    /// The `title` tag.
    pub title: String,
    /// The description — the event content.
    pub description: String,
    /// The streams, one `imeta` each.
    pub streams: Vec<Imeta>,
    /// Whether the event is addressable (`34235`/`34236`) and its
    /// `d` identifier.
    pub identifier: Option<String>,
}

/// Whether a MIME type names a video or audio stream NIP-71 carries.
#[must_use]
pub fn is_stream_type(media_type: &str) -> bool {
    media_type.starts_with("video/")
        || media_type.starts_with("audio/")
        || matches!(
            media_type,
            "application/x-mpegURL" | "application/vnd.apple.mpegurl"
        )
}

/// Read a video event: a `title`, at least one stream `imeta`, and
/// for the addressable kinds a `d` identifier.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, a missing
/// `title`, a malformed `imeta`, a missing `d` on an addressable
/// kind, or no stream.
pub fn open_video(event: &Event) -> Result<Video, DomainError> {
    let addressable = ADDRESSABLE_VIDEO_KINDS.contains(&event.kind);
    if !VIDEO_KINDS.contains(&event.kind) && !addressable {
        return Err(invalid("a video event is kind 21, 22, 34235, or 34236"));
    }
    let title = event
        .tag_values("title")
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("a video event requires a title tag"))?
        .to_string();
    let identifier = if addressable {
        Some(
            event
                .distinct_parameter()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| invalid("an addressable video requires a d tag"))?
                .to_string(),
        )
    } else {
        None
    };
    let streams: Vec<Imeta> = open_imetas(event)?
        .into_iter()
        .filter(|imeta| imeta.media_type().is_some_and(is_stream_type))
        .collect();
    if streams.is_empty() {
        return Err(invalid("a video event needs an imeta stream"));
    }
    for imeta in &streams {
        for key in ["duration", "bitrate"] {
            if let Some(value) = imeta.get(key)
                && value.parse::<f64>().is_err()
            {
                return Err(invalid("imeta duration and bitrate are numbers"));
            }
        }
        if let Some(waveform) = imeta.get("waveform")
            && !waveform
                .split_whitespace()
                .all(|value| value.parse::<u64>().is_ok())
        {
            return Err(invalid("an imeta waveform is space-separated integers"));
        }
    }
    Ok(Video {
        title,
        description: event.content.clone(),
        streams,
        identifier,
    })
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn sign(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, kind, tags, content.to_string())
    }

    #[test]
    fn a_video_event_carries_streams_and_its_address() {
        let video = sign(
            34_235,
            vec![
                Tag::new(vec!["d".into(), "summer-clip".into()]),
                Tag::new(vec!["title".into(), "Summer".into()]),
                Tag::new(vec![
                    "imeta".into(),
                    "url https://myvideo.com/1080/12345.mp4".into(),
                    "m video/mp4".into(),
                    "duration 29.223".into(),
                    "bitrate 3000000".into(),
                    format!("x {}", "ab".repeat(32)),
                ]),
                Tag::new(vec![
                    "imeta".into(),
                    "url https://myvideo.com/720/12345.m3u8".into(),
                    "m application/x-mpegURL".into(),
                    "duration 29.21".into(),
                ]),
                Tag::new(vec![
                    "text-track".into(),
                    "https://t.example/en.vtt".into(),
                    "captions".into(),
                    "en".into(),
                ]),
            ],
            "the summer cut",
        );
        let opened = open_video(&video).unwrap();
        assert_eq!(opened.title, "Summer");
        assert_eq!(opened.identifier.as_deref(), Some("summer-clip"));
        assert_eq!(opened.streams.len(), 2);
        assert_eq!(
            opened.streams[1].media_type(),
            Some("application/x-mpegURL")
        );
        assert!(is_stream_type("video/mp4"));
        assert!(is_stream_type("audio/mp3"));
        assert!(!is_stream_type("image/png"));

        let regular = sign(
            21,
            vec![
                Tag::new(vec!["title".into(), "v".into()]),
                Tag::new(vec![
                    "imeta".into(),
                    "url https://v.example/a.mp4".into(),
                    "m video/mp4".into(),
                ]),
            ],
            "x",
        );
        assert_eq!(open_video(&regular).unwrap().identifier, None);
    }

    #[test]
    fn malformed_videos_are_refused() {
        let no_d = sign(
            34_235,
            vec![
                Tag::new(vec!["title".into(), "t".into()]),
                Tag::new(vec![
                    "imeta".into(),
                    "url https://v.example/a.mp4".into(),
                    "m video/mp4".into(),
                ]),
            ],
            "x",
        );
        assert!(open_video(&no_d).is_err());
        let bad_duration = sign(
            21,
            vec![
                Tag::new(vec!["title".into(), "t".into()]),
                Tag::new(vec![
                    "imeta".into(),
                    "url https://v.example/a.mp4".into(),
                    "m video/mp4".into(),
                    "duration soon".into(),
                ]),
            ],
            "x",
        );
        assert!(open_video(&bad_duration).is_err());
        let image_only = sign(
            22,
            vec![
                Tag::new(vec!["title".into(), "t".into()]),
                Tag::new(vec![
                    "imeta".into(),
                    "url https://x/i.png".into(),
                    "m image/png".into(),
                ]),
            ],
            "x",
        );
        assert!(open_video(&image_only).is_err());
    }
}
