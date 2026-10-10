//! `coder export --account` (#11134): the client side of the website's
//! `/settings/export` (`openagents-web` `account_export`), which answers the
//! same single file Settings' Download button does when asked with Coder's
//! own sign-in (`coder login`).
//!
//! The file is kept as the website sent it: every chat with a Markdown copy,
//! projects, traces, computers, and settings, and never a key, token, or
//! password. It is written only to a new file (never over one that is
//! there), readable by this account's user alone.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use openagents_login::Saved;
use serde_json::Value;

/// Where the website answers the export.
pub const PATH: &str = "/settings/export";
/// What the file says it is.
pub const SCHEMA: &str = "openagents.account-export.v1";
/// The largest file read (chats plus up to 64 MiB of traces).
pub const MAX_BYTES: usize = 256 * 1024 * 1024;

/// The downloaded file.
#[derive(Debug)]
pub struct Export {
    /// The file as the website sent it.
    pub bytes: Vec<u8>,
    /// The name the website gave it, such as `openagents-export-2026-10-10.json`.
    pub file_name: String,
    pub chats: usize,
    pub traces: usize,
    /// Parts the website couldn't read this time, by name.
    pub unavailable: Vec<String>,
}

/// Today's date (UTC), `2026-10-10`, for a file the website didn't name.
#[must_use]
pub fn today() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
    atif::iso(ms).chars().take(10).collect()
}

/// The name in a `Content-Disposition: attachment; filename="…"`, when it
/// is a plain file name (no folders, no dots first).
#[must_use]
pub fn file_name_from(disposition: &str) -> Option<String> {
    let name = disposition
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("filename="))?
        .trim_matches('"');
    let plain = !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
    plain.then(|| name.to_owned())
}

/// The website's words for a refusal.
fn refusal(status: u16, body: &Value) -> String {
    if status == 401 || (300..400).contains(&status) {
        return "Your sign-in has ended. Run coder login, then try again.".into();
    }
    body["error"]["message"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if status >= 500 {
                "openagents.com couldn't make your export right now. Try again in a minute.".into()
            } else {
                format!("openagents.com refused the export ({status}).")
            }
        })
}

/// Read a downloaded file: it must be the account export.
///
/// # Errors
/// When it isn't one.
pub fn read(bytes: Vec<u8>, file_name: String) -> Result<Export, String> {
    let file: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "openagents.com answered in a way Coder doesn't understand.".to_string())?;
    if file["schema"] != SCHEMA {
        return Err("openagents.com answered in a way Coder doesn't understand.".into());
    }
    let count = |key: &str| file[key].as_array().map_or(0, Vec::len);
    Ok(Export {
        chats: count("chats"),
        traces: count("traces"),
        unavailable: file["unavailable"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        bytes,
        file_name,
    })
}

/// Download everything on the signed-in account.
///
/// # Errors
/// The website's words, or why it couldn't be reached.
pub fn download(saved: &Saved, today: &str) -> Result<Export, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Couldn't start the download.".to_string())?;
    runtime.block_on(async {
        let http = reqwest::Client::builder()
            // A redirect is the sign-in page: the token no longer signs in.
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|_| "Couldn't start the download.".to_string())?;
        let mut response = http
            .get(format!("{}{PATH}", saved.origin.trim_end_matches('/')))
            .bearer_auth(saved.token())
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|_| format!("Couldn't reach {}. Check your connection.", saved.origin))?;
        let status = response.status().as_u16();
        let name = response
            .headers()
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .and_then(file_name_from)
            .unwrap_or_else(|| format!("openagents-export-{today}.json"));
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "The download stopped partway. Try again.".to_string())?
        {
            bytes.extend_from_slice(&chunk);
            if bytes.len() > MAX_BYTES {
                return Err("The export is larger than Coder can save.".into());
            }
        }
        if !(200..300).contains(&status) {
            let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            return Err(refusal(status, &body));
        }
        read(bytes, name)
    })
}

/// Write `export` to `path`, a new file only this user can read.
///
/// # Errors
/// When the file is already there or can't be written.
pub fn save(export: &Export, path: &Path) -> Result<PathBuf, String> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            format!(
                "{} is already there. Choose another name with --output FILE.",
                path.display()
            )
        } else {
            format!("Couldn't write {}.", path.display())
        }
    })?;
    file.write_all(&export.bytes)
        .and_then(|()| file.flush())
        .map_err(|_| format!("Couldn't write {}.", path.display()))?;
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::Router;
    use axum::http::{HeaderMap, StatusCode, header};
    use axum::response::IntoResponse;
    use axum::routing::get;
    use serde_json::json;

    use super::*;

    const TOKEN: &str = "sess_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn saved(origin: &str) -> Saved {
        serde_json::from_value(json!({
            "origin": origin, "account": "acct_1", "label": "Octo",
            "expires_at": u64::MAX, "token": TOKEN,
        }))
        .unwrap()
    }

    fn serve(router: Router) -> String {
        let (send, address) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    send.send(format!("http://{}", listener.local_addr().unwrap()))
                        .unwrap();
                    axum::serve(listener, router).await.unwrap();
                });
        });
        address.recv().unwrap()
    }

    #[test]
    fn a_file_name_comes_only_from_a_plain_attachment_name() {
        assert_eq!(
            file_name_from("attachment; filename=\"openagents-export-2026-10-10.json\"").as_deref(),
            Some("openagents-export-2026-10-10.json")
        );
        for bad in [
            "attachment",
            "attachment; filename=\"\"",
            "attachment; filename=\"../../.bashrc\"",
            "attachment; filename=\"a/b.json\"",
            "attachment; filename=\".hidden\"",
        ] {
            assert_eq!(file_name_from(bad), None, "{bad}");
        }
        let today = today();
        assert_eq!(today.len(), 10);
        assert_eq!(today.as_bytes()[4], b'-');
    }

    #[test]
    fn the_export_is_asked_with_the_app_token_and_saved_once() {
        let seen: Arc<Mutex<Vec<String>>> = Arc::default();
        let kept = seen.clone();
        let file = json!({
            "schema": SCHEMA,
            "chats": [{"title": "One"}, {"title": "Two"}],
            "traces": [{"title": "Run"}],
            "unavailable": ["projects"],
        });
        let body = serde_json::to_vec_pretty(&file).unwrap();
        let router = Router::new().route(
            PATH,
            get(move |headers: HeaderMap| {
                let kept = kept.clone();
                let body = body.clone();
                async move {
                    kept.lock()
                        .unwrap()
                        .push(headers[header::AUTHORIZATION].to_str().unwrap().to_owned());
                    (
                        [
                            (header::CONTENT_TYPE, "application/json; charset=utf-8"),
                            (
                                header::CONTENT_DISPOSITION,
                                "attachment; filename=\"openagents-export-2026-10-10.json\"",
                            ),
                        ],
                        body,
                    )
                        .into_response()
                }
            }),
        );
        let origin = serve(router);
        let export = download(&saved(&origin), "1999-01-01").unwrap();
        assert_eq!(seen.lock().unwrap()[0], format!("Bearer {TOKEN}"));
        assert_eq!(export.file_name, "openagents-export-2026-10-10.json");
        assert_eq!((export.chats, export.traces), (2, 1));
        assert_eq!(export.unavailable, vec!["projects".to_owned()]);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(&export.file_name);
        save(&export, &path).unwrap();
        // The file is the website's, byte for byte, and private.
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(&path).unwrap()).unwrap(),
            file
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        // Never over a file that is there.
        let again = save(&export, &path).unwrap_err();
        assert!(again.contains("already there"), "{again}");
    }

    #[test]
    fn a_signed_out_token_and_a_stranger_answer_are_said_plainly() {
        let router = Router::new()
            .route(
                PATH,
                get(|| async {
                    (
                        StatusCode::UNAUTHORIZED,
                        axum::Json(json!({"error": {"code": "signed_out", "message": "x"}})),
                    )
                }),
            )
            .route(
                "/other/settings/export",
                get(|| async { axum::Json(json!({"schema": "something-else"})) }),
            );
        let origin = serve(router);
        assert_eq!(
            download(&saved(&origin), "2026-10-10").unwrap_err(),
            "Your sign-in has ended. Run coder login, then try again."
        );
        assert!(
            download(&saved(&format!("{origin}/other")), "2026-10-10")
                .unwrap_err()
                .contains("doesn't understand")
        );
        // A redirect (to the sign-in page) is a sign-in that has ended.
        assert!(refusal(303, &Value::Null).contains("coder login"));
        assert!(refusal(503, &Value::Null).contains("Try again"));
    }
}
