//! Everglade's Agent Studio, acting through a paired computer.
//!
//! The Verse tab's studio panels (`coder_mobile`'s studio panel) observe a
//! studio source. This module connects that source to one of the phone's
//! paired computers: the Computers service's supervised NIP-HOST link to
//! it, under the grant the phone holds for that computer. The panels then
//! offer what the grant's rights allow: answers and steering with
//! `operate`, and **Merge** and **Reject** with `review`. The computer
//! checks the grant again on every message.
//!
//! The host calls [`openagents_verse_studio_connect`] with its Verse handle,
//! its app handle, and the computer's host key, as it lists the computer
//! on the Computers screen. Text the person types into an open studio
//! panel goes to the Verse handle as `{"action":"studio_text","text":...}`.
use crate::{App, OpenAgentsMobileBuffer, buffer};
use coder_mobile::VerseHandle;
use serde::Serialize;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// The longest host key the call takes: a 64-character public key, with
/// room for a padded copy.
const MAX_HOST_BYTES: usize = 256;

/// What [`openagents_verse_studio_connect`] answers, as JSON.
#[derive(Debug, PartialEq, Serialize)]
struct Reply {
    connected: bool,
    /// The grant's rights, such as `observe`, `operate`, and `review`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rights: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Reply {
    fn of(result: Result<Vec<&'static str>, String>) -> Self {
        match result {
            Ok(rights) => Self {
                connected: true,
                rights,
                error: None,
            },
            Err(error) => Self {
                connected: false,
                rights: Vec::new(),
                error: Some(error),
            },
        }
    }
}

/// Connects `verse`'s Everglade studio to the paired computer `host`
/// through `app`'s live Computers client. Returns the grant's rights.
///
/// # Errors
/// A plain sentence: no computer is named, the app has no live Computers
/// client, or the computer is not connected now.
pub(crate) fn connect(
    verse: &mut VerseHandle,
    app: &App,
    host: &str,
) -> Result<Vec<&'static str>, String> {
    let host = host.trim();
    if host.is_empty() {
        return Err("Choose a computer for the studio".into());
    }
    let (links, runtime) = app
        .studio_links(host)
        .ok_or("This phone has no live connection to its computers")?;
    let rights = verse
        .connect_studio(links, runtime)
        .map_err(|error| coder_computers::describe(&error))?;
    Ok(rights.into_iter().map(|right| right.as_str()).collect())
}

/// Connects the Verse handle's Everglade studio to the paired computer
/// whose host key is the UTF-8 `host`, under the grant `app` holds for it.
/// Answers `{"connected":true,"rights":[...]}`, or `{"connected":false,
/// "error":"..."}`; free the buffer with `openagents_mobile_buffer_free`.
/// The studio starts reading when the player enters Everglade.
///
/// # Safety
/// `verse` and `app` are live handles from their create calls, used on the
/// main thread, and `host` points to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn openagents_verse_studio_connect(
    verse: *mut VerseHandle,
    app: *mut App,
    host: *const u8,
    len: usize,
) -> OpenAgentsMobileBuffer {
    if verse.is_null() || app.is_null() || host.is_null() || len == 0 || len > MAX_HOST_BYTES {
        return buffer(vec![]);
    }
    catch_unwind(AssertUnwindSafe(|| {
        let host = unsafe { std::slice::from_raw_parts(host, len) };
        let result = match std::str::from_utf8(host) {
            Ok(host) => connect(unsafe { &mut *verse }, unsafe { &*app }, host),
            Err(_) => Err("The computer's key is not text".into()),
        };
        serde_json::to_vec(&Reply::of(result)).unwrap_or_default()
    }))
    .map(buffer)
    .unwrap_or_else(|_| buffer(vec![]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reply_names_the_rights_or_the_reason() {
        let connected = serde_json::to_value(Reply::of(Ok(vec!["observe", "operate"]))).unwrap();
        assert_eq!(
            connected,
            serde_json::json!({"connected": true, "rights": ["observe", "operate"]})
        );
        let refused =
            serde_json::to_value(Reply::of(Err("this computer is not connected".into()))).unwrap();
        assert_eq!(
            refused,
            serde_json::json!({"connected": false, "error": "this computer is not connected"})
        );
    }
}
