//! Photos and videos in a project's Drive sources, looked at by Gemini on
//! Vertex AI ("Google first"): the host's
//! [`oa_connections::google::drive::LookAt`].
//!
//! - The model is Gemini 4 Argon ([`MODEL`]), or the chat's Gemini door
//!   when this server doesn't list Argon. Videos are sent at Gemini's low
//!   media resolution, which keeps their tokens (the cost) down.
//! - A file up to [`INLINE_BYTES`] goes inside the request. A larger one
//!   (a video, up to the Drive door's caps) is streamed from Drive into a
//!   private object in [`bucket`], named at random, handed to Gemini as a
//!   `gs://` URI, and deleted as soon as Gemini has answered or failed.
//!   The bucket allows no public access, keeps no soft-deleted copies,
//!   and deletes anything a day old, should a delete ever fail.
//! - Nothing here logs a file's name or what it holds, and nothing of it
//!   is kept after the request.
//! - Each look returns its description and what it cost ([`spent`]).

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use base64::Engine;
use futures_util::StreamExt;
use oa_connections::google::drive::{Drive, LookAt, Media, MediaKind, Seen};
use serde_json::{Value, json};

use super::chat::{gemini_call_within, spent};
use crate::chat_vision::{Gemini, gemini_text};

/// The model that looks at photos and videos.
pub(crate) const MODEL: &str = "google/gemini-4-argon";
/// A file this size or smaller goes to Gemini inside the request.
pub(crate) const INLINE_BYTES: u64 = 7 * 1024 * 1024;
/// How long one look may take (a long video takes Gemini a while).
const LOOK_DEADLINE: Duration = Duration::from_secs(150);
/// The Cloud Storage JSON API.
const STORAGE_API: &str = "https://storage.googleapis.com";

/// What Gemini is asked about each photo or video.
fn prompt(kind: MediaKind) -> String {
    let heard = match kind {
        MediaKind::Image => "",
        MediaKind::Video => " For a video, say what happens in order, and what is said or heard.",
    };
    format!(
        "Describe what is in this {} for someone who can't see it: the setting, the people \
(by appearance and what they do, never who they are), the objects, and any text that can be read.{heard} \
Be concrete and brief, at most 150 words. Describe only what is there.",
        kind.word()
    )
}

/// The private bucket larger files pass through: `OPENAGENTS_WEB_DRIVE_MEDIA_BUCKET`,
/// or `{VERTEX_PROJECT}-drive-media-tmp`.
pub(crate) fn bucket() -> String {
    let var = |name: &str| {
        std::env::var(name)
            .ok()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    var("OPENAGENTS_WEB_DRIVE_MEDIA_BUCKET").unwrap_or_else(|| {
        format!(
            "{}-drive-media-tmp",
            var("VERTEX_PROJECT").unwrap_or_else(|| "openagentsgemini".to_owned())
        )
    })
}

/// The Gemini that looks: Argon when this server lists it, else the chat's.
fn gemini() -> Option<Gemini> {
    static ARGON: OnceLock<Option<Gemini>> = OnceLock::new();
    ARGON
        .get_or_init(|| Gemini::from_env_for(MODEL))
        .clone()
        .or_else(|| crate::chat_vision::doors_gemini())
}

/// Photos and videos described, when this server has Gemini.
pub(crate) fn looker() -> Option<LookAt> {
    let gemini = gemini()?;
    let bucket = bucket();
    Some(Arc::new(move |drive: Drive, media: Media| {
        let gemini = gemini.clone();
        let bucket = bucket.clone();
        Box::pin(async move { look(&gemini, &bucket, &drive, &media).await })
    }))
}

/// The `generateContent` body asking about one file in `part`.
pub(crate) fn body(kind: MediaKind, part: Value) -> Value {
    let mut generation = json!({
        "maxOutputTokens": 2048,
        "thinkingConfig": {"thinkingLevel": "low"},
    });
    if kind == MediaKind::Video {
        generation["mediaResolution"] = json!("MEDIA_RESOLUTION_LOW");
    }
    json!({
        "contents": [{"role": "user", "parts": [part, {"text": prompt(kind)}]}],
        "generationConfig": generation,
    })
}

/// A random object name: never the file's.
fn object_name() -> String {
    let bytes: [u8; 16] = secp256k1::rand::random();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("drive-media/{hex}")
}

/// Look at `media`: inline when small, through a short-lived private
/// object when not.
async fn look(gemini: &Gemini, bucket: &str, drive: &Drive, media: &Media) -> Result<Seen, String> {
    let started = std::time::Instant::now();
    let seen = look_now(gemini, bucket, drive, media).await;
    // Its kind, size, and time only: never its name or what it holds.
    eprintln!(
        "openagents-web: drive media: a {} of {} MB {} in {} ms",
        media.kind.word(),
        media.bytes.unwrap_or(0) / (1024 * 1024),
        if seen.is_ok() { "looked at" } else { "failed" },
        started.elapsed().as_millis()
    );
    seen
}

async fn look_now(
    gemini: &Gemini,
    bucket: &str,
    drive: &Drive,
    media: &Media,
) -> Result<Seen, String> {
    let mime = media.file.mime_type.clone();
    let small = media.bytes.is_some_and(|bytes| bytes <= INLINE_BYTES);
    if small {
        let bytes = drive
            .media_bytes(&media.file.id, INLINE_BYTES as usize)
            .await
            .map_err(|error| error.to_string())?;
        let part = json!({"inlineData": {
            "mimeType": mime,
            "data": base64::engine::general_purpose::STANDARD.encode(&bytes),
        }});
        drop(bytes);
        return ask(gemini, &body(media.kind, part)).await;
    }
    let cap = match media.kind {
        MediaKind::Image => oa_connections::google::drive::MAX_IMAGE_BYTES,
        MediaKind::Video => oa_connections::google::drive::MAX_VIDEO_BYTES,
    };
    let object = object_name();
    let uploaded = upload(gemini, bucket, &object, drive, media, &mime, cap).await;
    let seen = match uploaded {
        Ok(()) => {
            let part = json!({"fileData": {
                "mimeType": mime,
                "fileUri": format!("gs://{bucket}/{object}"),
            }});
            ask(gemini, &body(media.kind, part)).await
        }
        Err(why) => Err(why),
    };
    // Deleted whatever happened; a partial upload leaves no object, and
    // the bucket's one-day rule catches a delete that fails.
    remove(gemini, bucket, &object).await;
    seen
}

/// One Gemini call: the description and what it cost.
async fn ask(gemini: &Gemini, body: &Value) -> Result<Seen, String> {
    let reply = gemini_call_within(gemini, body, LOOK_DEADLINE).await?;
    let text = gemini_text(&reply).ok_or("the model said nothing about it")?;
    Ok(Seen {
        text,
        spent: spent(&gemini.row, &reply),
    })
}

/// Stream the file from Drive into `bucket/object`, stopping past `cap`.
async fn upload(
    gemini: &Gemini,
    bucket: &str,
    object: &str,
    drive: &Drive,
    media: &Media,
    mime: &str,
    cap: u64,
) -> Result<(), String> {
    let response = drive
        .media_stream(&media.file.id, cap)
        .await
        .map_err(|error| error.to_string())?;
    let mut sent: u64 = 0;
    let stream = response.bytes_stream().map(move |chunk| {
        let chunk = chunk.map_err(|_| std::io::Error::other("Drive stopped sending it"))?;
        sent += chunk.len() as u64;
        if sent > cap {
            return Err(std::io::Error::other("it is larger than it said"));
        }
        Ok::<_, std::io::Error>(chunk)
    });
    let token = gemini.token.token().await?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(150))
        .build()
        .map_err(|error| error.to_string())?;
    let mut request = client
        .post(format!("{STORAGE_API}/upload/storage/v1/b/{bucket}/o"))
        .query(&[
            ("uploadType", "media"),
            ("name", object),
            ("ifGenerationMatch", "0"),
            ("fields", "generation"),
        ])
        .bearer_auth(token.expose())
        .header(reqwest::header::CONTENT_TYPE, mime);
    if let Some(bytes) = media.bytes {
        request = request.header(reqwest::header::CONTENT_LENGTH, bytes);
    }
    let response = request
        .body(reqwest::Body::wrap_stream(stream))
        .send()
        .await
        .map_err(|_| "it couldn't be passed to the model".to_owned())?;
    if !response.status().is_success() {
        return Err(format!(
            "it couldn't be passed to the model (storage answered {})",
            response.status().as_u16()
        ));
    }
    Ok(())
}

/// Delete `bucket/object`; a failure is logged without any file's name.
async fn remove(gemini: &Gemini, bucket: &str, object: &str) {
    let Ok(token) = gemini.token.token().await else {
        eprintln!("openagents-web: drive media: no token to delete a passed-through object");
        return;
    };
    let name: String = url::form_urlencoded::byte_serialize(object.as_bytes()).collect();
    let deleted = reqwest::Client::new()
        .delete(format!("{STORAGE_API}/storage/v1/b/{bucket}/o/{name}"))
        .bearer_auth(token.expose())
        .timeout(Duration::from_secs(20))
        .send()
        .await;
    match deleted {
        Ok(response) if response.status().is_success() || response.status().as_u16() == 404 => {}
        Ok(response) => eprintln!(
            "openagents-web: drive media: delete answered {}; the bucket's one-day rule removes it",
            response.status().as_u16()
        ),
        Err(_) => eprintln!(
            "openagents-web: drive media: delete failed; the bucket's one-day rule removes it"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn videos_go_at_low_resolution_and_the_prompt_never_names_the_file() {
        let video = body(
            MediaKind::Video,
            json!({"fileData": {"mimeType": "video/quicktime", "fileUri": "gs://b/drive-media/x"}}),
        );
        assert_eq!(
            video["generationConfig"]["mediaResolution"],
            "MEDIA_RESOLUTION_LOW"
        );
        let words = video["contents"][0]["parts"][1]["text"].as_str().unwrap();
        assert!(words.contains("video") && words.contains("heard"));
        let photo = body(MediaKind::Image, json!({"inlineData": {}}));
        assert!(photo["generationConfig"]["mediaResolution"].is_null());
        let name = object_name();
        assert!(name.starts_with("drive-media/") && name.len() == "drive-media/".len() + 32);
        assert_ne!(name, object_name());
    }

    /// The whole look against Vertex and Cloud Storage, with a fake Drive
    /// serving the video at `OA_LIVE_VIDEO` (make it larger than
    /// [`INLINE_BYTES`] to go through the bucket): run with a Google
    /// credential that may call Vertex and write the bucket.
    #[tokio::test]
    #[ignore = "calls Vertex AI and Cloud Storage"]
    async fn live_gemini_describes_a_large_video_through_storage() {
        use oa_connections::fake::{self, Content, FakeFile, FakeGoogle};
        let path = std::env::var("OA_LIVE_VIDEO").expect("OA_LIVE_VIDEO");
        let bytes = std::fs::read(path).unwrap();
        let size = bytes.len() as u64;
        let google = FakeGoogle::start(
            vec![FakeFile::new(
                "video-live-0000001",
                "clip.mov",
                None,
                Content::Media("video/quicktime".into(), bytes, Some(10_000)),
            )],
            &[oa_connections::google::DRIVE_READONLY],
        )
        .await;
        let drive = Drive::new(
            reqwest::Client::new(),
            google.endpoints(),
            fake::ACCESS.into(),
            false,
        );
        let file = drive.file("video-live-0000001").await.unwrap();
        let gemini = gemini().expect("a Vertex credential");
        let seen = look(
            &gemini,
            &bucket(),
            &drive,
            &Media {
                kind: MediaKind::Video,
                file,
                bytes: Some(size),
            },
        )
        .await
        .unwrap();
        eprintln!(
            "{} ({:?}, through storage: {})",
            seen.text,
            seen.spent,
            size > INLINE_BYTES
        );
        assert!(!seen.text.is_empty() && seen.spent.input > 0);
    }
}
