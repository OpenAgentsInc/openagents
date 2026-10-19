//! Files the panel can't show as text or an image: a card with the file's
//! name and size, a Download button that copies it into this computer's
//! Downloads folder over the same device channel the panel reads through,
//! and Open when the file is already on this computer.
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use gpui::{AnyElement, Context, SharedString, div, prelude::*, px};
use openagents_proto::{WorkspaceFileText, WorkspaceReadOnlyReason};

use super::{
    FilesSurface,
    client::{FilesClientError, WorkspaceFilesClient},
};
use crate::{
    icons::{self, icon},
    theme::Theme,
};

/// What the panel shows for a path once its read finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreviewChoice {
    /// Rendered from bytes fetched on the owning computer.
    Image,
    /// The text editor or read-only text preview.
    Text,
    /// A name and size card with Download (and Open on this computer).
    Download,
    /// A one-line reason, such as a symlink or a permission error.
    Message,
}

pub(super) fn preview_choice(path: &str, file: Option<&WorkspaceFileText>) -> PreviewChoice {
    if super::image_preview::is_image(path) {
        return PreviewChoice::Image;
    }
    match file {
        Some(file) if file.text.is_some() => PreviewChoice::Text,
        Some(file)
            if matches!(
                file.read_only_reason,
                Some(WorkspaceReadOnlyReason::Binary) | Some(WorkspaceReadOnlyReason::TooLarge)
            ) =>
        {
            PreviewChoice::Download
        }
        _ => PreviewChoice::Message,
    }
}

#[derive(Default)]
pub(super) struct DownloadProgress {
    received: AtomicU64,
    total: AtomicU64,
}

pub(super) enum DownloadState {
    Running(Arc<DownloadProgress>),
    Saved(PathBuf),
    Failed(SharedString),
}

/// The base name of a workspace or absolute path, never empty.
fn file_name(path: &str) -> String {
    path.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("download")
        .to_string()
}

/// `name`, or `name (1)`, `name (2)`, ... before the extension, whichever
/// `exists` says is free.
pub(super) fn unique_destination(
    dir: &Path,
    name: &str,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    let first = dir.join(name);
    if !exists(&first) {
        return first;
    }
    let (stem, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    };
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}){extension}")))
        .find(|candidate| !exists(candidate))
        .expect("an unbounded range finds a free name")
}

pub(super) fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

fn downloads_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        None
    }
    #[cfg(not(test))]
    {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(|home| PathBuf::from(home).join("Downloads"))
    }
}

fn failure_message(error: &FilesClientError) -> SharedString {
    match error {
        FilesClientError::Unsupported(_) => {
            "This computer needs an update before it can send files.".into()
        }
        FilesClientError::Request(message) if message == "file not found" => {
            "This file is no longer there.".into()
        }
        _ => "Couldn't download this file. Try again.".into(),
    }
}

const REVEAL_LABEL: &str = if cfg!(target_os = "macos") {
    "Show in Finder"
} else {
    "Show in folder"
};

impl FilesSurface {
    /// Copy `path` into Downloads and reveal it there. A download already
    /// running for the path keeps going.
    pub fn download_file(&mut self, path: String, cx: &mut Context<Self>) {
        if matches!(self.downloads.get(&path), Some(DownloadState::Running(_))) {
            return;
        }
        let Some(context) = self.request_context.clone() else {
            return;
        };
        let Some(engine) = self.state.read(cx).engine().cloned() else {
            self.downloads.insert(
                path,
                DownloadState::Failed("Still connecting. Try again in a moment.".into()),
            );
            cx.notify();
            return;
        };
        let Some(dir) = downloads_dir() else {
            self.downloads.insert(
                path,
                DownloadState::Failed("Couldn't find your Downloads folder.".into()),
            );
            cx.notify();
            return;
        };
        let client = WorkspaceFilesClient::new(engine, context);
        let progress = Arc::new(DownloadProgress::default());
        self.downloads
            .insert(path.clone(), DownloadState::Running(progress.clone()));
        let job_path = path.clone();
        let job_progress = progress.clone();
        let job =
            cx.background_spawn(
                async move { save_download(client, job_path, dir, job_progress).await },
            );
        let task_path = path.clone();
        let task = cx.spawn(async move |this, cx| {
            let mut job = Box::pin(job);
            let result = loop {
                let tick = cx.background_executor().timer(Duration::from_millis(200));
                match futures::future::select(job.as_mut(), tick).await {
                    futures::future::Either::Left((result, _)) => break result,
                    futures::future::Either::Right(_) => {
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            return;
                        }
                    }
                }
            };
            let _ = this.update(cx, |surface, cx| {
                surface.download_tasks.remove(&task_path);
                let state = match result {
                    Ok(saved) => {
                        cx.reveal_path(&saved);
                        DownloadState::Saved(saved)
                    }
                    Err(error) => DownloadState::Failed(error),
                };
                surface.downloads.insert(task_path, state);
                cx.notify();
            });
        });
        self.download_tasks.insert(path, task);
        cx.notify();
    }

    pub(super) fn render_download_card(
        &mut self,
        path: &str,
        file: &WorkspaceFileText,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = file_name(path);
        let local_path = self.system_open_path(path);
        let state = self.downloads.get(path);
        let running = matches!(state, Some(DownloadState::Running(_)));
        let button = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .h(px(28.0))
                .px(px(12.0))
                .rounded(px(6.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.0))
                .cursor_pointer()
                .role(gpui::Role::Button)
                .aria_label(label)
                .when(primary, |el| el.bg(theme.solid).text_color(theme.on_solid))
                .when(!primary, |el| {
                    el.border_1()
                        .border_color(theme.border)
                        .text_color(theme.text)
                        .hover(|style| style.bg(crate::theme::wash(0.08)))
                })
                .child(label)
        };
        let download_path = path.to_string();
        let mut actions =
            div().flex().gap(px(8.0)).child(
                button("files-download", "Download", true)
                    .when(running, |el| el.opacity(0.5))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.download_file(download_path.clone(), cx)
                    })),
            );
        if let Some(local_path) = local_path {
            actions = actions.child(
                button("files-open-local", "Open", false)
                    .on_click(move |_, _, cx| cx.open_with_system(&local_path)),
            );
        }
        let status: Option<AnyElement> = match state {
            Some(DownloadState::Running(progress)) => {
                let total = progress.total.load(Ordering::Relaxed);
                let received = progress.received.load(Ordering::Relaxed);
                let label = if total > 0 {
                    format!("Downloading… {}%", received * 100 / total)
                } else {
                    "Downloading…".to_string()
                };
                Some(
                    div()
                        .text_size(px(11.5))
                        .text_color(theme.text_muted)
                        .child(label)
                        .into_any_element(),
                )
            }
            Some(DownloadState::Saved(saved)) => {
                let saved_name = saved
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let reveal = saved.clone();
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(theme.text_muted)
                                .child(format!("Saved to Downloads as {saved_name}.")),
                        )
                        .child(
                            button("files-download-reveal", REVEAL_LABEL, false)
                                .on_click(move |_, _, cx| cx.reveal_path(&reveal)),
                        )
                        .into_any_element(),
                )
            }
            Some(DownloadState::Failed(message)) => Some(
                div()
                    .text_size(px(11.5))
                    .text_color(theme.danger_muted)
                    .child(message.clone())
                    .into_any_element(),
            ),
            None => None,
        };
        div()
            .id("files-download-card")
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .px(px(24.0))
            .text_center()
            .child(
                icon(icons::DOCUMENT)
                    .size(px(28.0))
                    .text_color(theme.text_muted),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(theme.text)
                    .max_w_full()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(name),
            )
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(theme.text_faint)
                    .child(format_size(file.size)),
            )
            .child(actions)
            .children(status)
            .into_any_element()
    }
}

/// Stream the file into a hidden partial file beside its destination, then
/// move it to a free name in `dir`. Errors come back as text for the card.
async fn save_download(
    client: WorkspaceFilesClient,
    path: String,
    dir: PathBuf,
    progress: Arc<DownloadProgress>,
) -> Result<PathBuf, SharedString> {
    let fail = |error: &FilesClientError| {
        tracing::warn!(%path, %error, "workspace file download failed");
        failure_message(error)
    };
    let local_error = |error: std::io::Error| -> SharedString {
        tracing::warn!(%error, "couldn't write a download");
        "Couldn't save to your Downloads folder.".into()
    };
    let checkout = client.checkout_for(&path).await.map_err(|e| fail(&e))?;
    std::fs::create_dir_all(&dir).map_err(local_error)?;
    let name = file_name(&path);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or_default();
    let partial = dir.join(format!(".{name}.{nonce}.download"));
    let file = std::fs::File::create(&partial).map_err(local_error)?;
    let mut sink = std::io::BufWriter::new(file);
    let report = |received: u64, total: u64| {
        progress.total.store(total, Ordering::Relaxed);
        progress.received.store(received, Ordering::Relaxed);
    };
    let result = client
        .download(path.clone(), checkout, &mut sink, &report)
        .await;
    let finished = match result {
        Ok(_) => std::io::Write::flush(&mut sink).map_err(local_error),
        Err(error) => Err(fail(&error)),
    };
    drop(sink);
    if let Err(message) = finished {
        let _ = std::fs::remove_file(&partial);
        return Err(message);
    }
    let destination = unique_destination(&dir, &name, |candidate| candidate.exists());
    if let Err(error) = std::fs::rename(&partial, &destination) {
        let _ = std::fs::remove_file(&partial);
        return Err(local_error(error));
    }
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_proto::WorkspaceTextEncoding;

    fn read(text: Option<&str>, reason: Option<WorkspaceReadOnlyReason>) -> WorkspaceFileText {
        WorkspaceFileText {
            checkout_id: "checkout".into(),
            path: "x".into(),
            text: text.map(Into::into),
            content_hash: None,
            size: 2048,
            modified_at: None,
            encoding: if text.is_some() {
                WorkspaceTextEncoding::Utf8
            } else {
                WorkspaceTextEncoding::Binary
            },
            line_ending: None,
            read_only_reason: reason,
            truncated: false,
        }
    }

    #[test]
    fn panel_renders_images_shows_text_and_offers_other_binaries_for_download() {
        // Images render on any computer, before and after a text read.
        for path in [
            "/home/me/scratch/foreman-agent.png",
            "shots/a.JPG",
            "b.gif",
            "c.webp",
        ] {
            assert_eq!(preview_choice(path, None), PreviewChoice::Image);
        }
        assert_eq!(
            preview_choice("notes.txt", Some(&read(Some("hi"), None))),
            PreviewChoice::Text
        );
        let binary = read(None, Some(WorkspaceReadOnlyReason::Binary));
        for path in [
            "/home/me/scratch/foreman-screenshots.zip",
            "report.pdf",
            "clip.mov",
        ] {
            assert_eq!(preview_choice(path, Some(&binary)), PreviewChoice::Download);
        }
        assert_eq!(
            preview_choice(
                "huge.log",
                Some(&read(None, Some(WorkspaceReadOnlyReason::TooLarge)))
            ),
            PreviewChoice::Download
        );
        assert_eq!(
            preview_choice(
                "link",
                Some(&read(None, Some(WorkspaceReadOnlyReason::Symlink)))
            ),
            PreviewChoice::Message
        );
        assert_eq!(preview_choice("pending.zip", None), PreviewChoice::Message);
    }

    #[test]
    fn downloads_never_overwrite_and_sizes_read_plainly() {
        let dir = Path::new("/d");
        let taken = ["/d/shots.zip", "/d/shots (1).zip", "/d/README"];
        let exists = |p: &Path| taken.iter().any(|t| Path::new(t) == p);
        assert_eq!(
            unique_destination(dir, "shots.zip", exists),
            dir.join("shots (2).zip")
        );
        assert_eq!(
            unique_destination(dir, "README", exists),
            dir.join("README (1)")
        );
        assert_eq!(
            unique_destination(dir, "new.png", exists),
            dir.join("new.png")
        );
        assert_eq!(
            unique_destination(dir, ".env", |p| p == Path::new("/d/.env")),
            dir.join(".env (1)")
        );
        assert_eq!(
            file_name("/home/me/scratch/foreman-screenshots.zip"),
            "foreman-screenshots.zip"
        );
        assert_eq!(format_size(812), "812 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(5 * 1024 * 1024 + 300_000), "5.3 MB");
        assert_eq!(format_size(300 * 1024 * 1024), "300 MB");
    }

    #[test]
    fn links_that_say_download_start_one() {
        use super::super::link_asks_for_download as asks;
        assert!(asks("Download all nine screenshots and full instructions"));
        assert!(asks(" download the zip"));
        assert!(!asks("Foreman in Agent Lab"));
        assert!(!asks("Downloa"));
        assert!(!asks("é"));
    }

    #[test]
    fn card_copy_is_plain() {
        let texts = [
            "Download",
            "Open",
            REVEAL_LABEL,
            "Downloading…",
            "Saved to Downloads as shots.zip.",
            "This computer needs an update before it can send files.",
            "This file is no longer there.",
            "Couldn't download this file. Try again.",
            "Couldn't save to your Downloads folder.",
            "Couldn't find your Downloads folder.",
            "Still connecting. Try again in a moment.",
        ];
        for text in texts {
            assert!(!text.to_ascii_lowercase().contains("zeron"), "{text}");
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }
}
