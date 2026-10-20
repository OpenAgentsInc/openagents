//! Bounded, lazy previews for the desktop transcript hosted in a browser.
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use gpui::{BackgroundExecutor, Context, Image, ImageFormat, SharedString, Task};
use openagents_chat_view::rpc::{Rpc, call_with_timeout};
use openagents_chat_view::tool_image_paths::resolve_path;
use openagents_chat_view::transcript::{FOLD_TWEEN_WINDOW, FoldState, Row, ToolItem};
use openagents_proto::WorkspaceImageChunk;
use serde_json::json;

use crate::desktop_transcript::Transcript;

const MAX_IMAGES: usize = 16;
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub enum Preview {
    Loading,
    Ready(Arc<Image>),
    Failed(SharedString),
}

struct Cached {
    preview: Preview,
    bytes: usize,
    _task: Option<Task<()>>,
}

#[derive(Default)]
pub(crate) struct Previews {
    chips: HashMap<String, Vec<String>>,
    images: HashMap<String, Cached>,
    generation: u64,
}

impl Previews {
    pub fn clear(&mut self) {
        self.chips.clear();
        self.images.clear();
        self.generation = self.generation.wrapping_add(1);
    }

    fn prune(&mut self) {
        let held: HashSet<&str> = self.chips.values().flatten().map(String::as_str).collect();
        self.images.retain(|path, _| held.contains(path.as_str()));
    }

    pub fn release_row(&mut self, row: &str) {
        let prefix = format!("{row}#d");
        self.chips.retain(|chip, _| !chip.starts_with(&prefix));
        self.prune();
    }

    pub fn retain_rows(&mut self, rows: &[Row]) {
        let prefixes: Vec<_> = rows.iter().map(|row| format!("{}#d", row.id)).collect();
        self.chips
            .retain(|chip, _| prefixes.iter().any(|prefix| chip.starts_with(prefix)));
        self.prune();
    }
}

pub(crate) fn hold(
    this: &mut Transcript,
    row: &SharedString,
    tools: &[ToolItem],
    opens: &[bool],
    folds: &[FoldState],
    cx: &mut Context<Transcript>,
) -> Vec<Option<Vec<Preview>>> {
    let place = this.host.chat_place(this.chat_id.as_deref(), cx);
    let rpc = this.host.rpc(cx);
    let chat = this.chat_id.clone();
    let mut paths = Vec::with_capacity(tools.len());
    for (ix, tool) in tools.iter().enumerate() {
        let chip = format!("{row}#d{ix}");
        let tweening = folds[ix]
            .toggled_at
            .is_some_and(|at| at.elapsed() < FOLD_TWEEN_WINDOW);
        if !opens[ix] && !tweening {
            this.media.previews.chips.remove(&chip);
            paths.push(Vec::new());
            continue;
        }
        let images: Vec<String> = tool
            .images
            .iter()
            .take(MAX_IMAGES)
            .map(|path| resolve_path(path, place.as_ref().and_then(|place| place.cwd.as_deref())))
            .collect();
        this.media.previews.chips.insert(chip, images.clone());
        paths.push(images);
    }
    this.media.previews.prune();
    for path in paths.iter().flatten() {
        let previews = &this.media.previews;
        if previews.images.contains_key(path) || previews.images.len() >= MAX_IMAGES {
            continue;
        }
        // Bound concurrent reads as well as resident decoded images.
        if previews
            .images
            .values()
            .filter(|image| matches!(image.preview, Preview::Loading))
            .count()
            >= 2
        {
            continue;
        }
        let (Some(rpc), Some(chat)) = (rpc.clone(), chat.clone()) else {
            continue;
        };
        let path = path.clone();
        let key = path.clone();
        let generation = previews.generation;
        let task = cx.spawn(async move |this, cx| {
            let loaded = read_image(&rpc, &chat, &path, cx.background_executor()).await;
            this.update(cx, |this, cx| {
                let previews = &mut this.media.previews;
                if previews.generation != generation || !previews.images.contains_key(&path) {
                    return;
                }
                let resident: usize = previews.images.values().map(|image| image.bytes).sum();
                let (preview, bytes) = match loaded {
                    Ok((image, bytes)) if resident + bytes <= MAX_BYTES => {
                        (Preview::Ready(image), bytes)
                    }
                    Ok(_) => (
                        Preview::Failed(
                            "Image preview limit reached. Close another tool to free space.".into(),
                        ),
                        0,
                    ),
                    Err(error) => (
                        Preview::Failed(
                            format!("{error}. Close and reopen the tool to retry.").into(),
                        ),
                        0,
                    ),
                };
                previews.images.insert(
                    path,
                    Cached {
                        preview,
                        bytes,
                        _task: None,
                    },
                );
                cx.notify();
            })
            .ok();
        });
        this.media.previews.images.insert(
            key,
            Cached {
                preview: Preview::Loading,
                bytes: 0,
                _task: Some(task),
            },
        );
    }
    paths
        .into_iter()
        .map(|paths| {
            (!paths.is_empty()).then(|| {
                paths
                    .iter()
                    .map(|path| {
                        this.media
                            .previews
                            .images
                            .get(path)
                            .map(|cached| cached.preview.clone())
                            .unwrap_or_else(|| {
                                if this.media.previews.images.len() >= MAX_IMAGES {
                                    Preview::Failed("Image preview limit reached".into())
                                } else {
                                    Preview::Loading
                                }
                            })
                    })
                    .collect()
            })
        })
        .collect()
}

async fn read_image(
    rpc: &Rpc,
    chat: &str,
    path: &str,
    executor: &BackgroundExecutor,
) -> anyhow::Result<(Arc<Image>, usize)> {
    #[cfg(target_family = "wasm")]
    let (bytes, mime) = {
        let image = call_with_timeout(
            rpc,
            executor,
            "ReadTranscriptImage",
            json!({"chatId": chat, "path": path}),
            Duration::from_secs(60),
        )
        .await
        .map_err(anyhow::Error::msg)?;
        let data = image["data"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Image read was incomplete"))?;
        let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
        let mime = image["mimeType"].as_str().unwrap_or_default().to_owned();
        (bytes, mime)
    };
    #[cfg(not(target_family = "wasm"))]
    let (bytes, mime) = read_image_bytes(
        |method, params| async move {
            call_with_timeout(rpc, executor, &method, params, Duration::from_secs(20))
                .await
                .map_err(anyhow::Error::msg)
        },
        chat,
        path,
    )
    .await?;
    let format = match mime.as_str() {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::Webp,
        "image/bmp" => ImageFormat::Bmp,
        "image/svg+xml" => ImageFormat::Svg,
        _ => anyhow::bail!("Image format isn't supported"),
    };
    let size = bytes.len();
    Ok((Arc::new(Image::from_bytes(format, bytes)), size))
}

/// Read one bounded image, retaining checkout and content identity across chunks.
/// Linked by the lazy extras module; the initial chat requests its result.
pub async fn read_image_bytes<F, Fut>(
    call: F,
    chat: &str,
    path: &str,
) -> anyhow::Result<(Vec<u8>, String)>
where
    F: Fn(String, serde_json::Value) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<serde_json::Value>>,
{
    let page = call(
        "ListWorkspaceDirectory".to_owned(),
        json!({
            "chatId": chat, "directory": "", "includeIgnored": false,
        }),
    )
    .await?;
    let checkout = page["checkoutId"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("The chat's folder changed"))?;
    let mut bytes = Vec::new();
    let mut offset = 0;
    let mut hash = None::<String>;
    let mut mime;
    loop {
        let chunk: WorkspaceImageChunk = serde_json::from_value(
            call(
                "ReadWorkspaceImage".to_owned(),
                json!({"chatId": chat, "path": path,
                "expectedCheckoutId": checkout, "offset": offset, "expectedContentHash": hash}),
            )
            .await?,
        )?;
        anyhow::ensure!(chunk.checkout_id == checkout, "The chat's folder changed");
        anyhow::ensure!(chunk.size <= 8 * 1024 * 1024, "Image is too large");
        anyhow::ensure!(
            hash.as_ref().is_none_or(|hash| hash == &chunk.content_hash),
            "Image changed while loading"
        );
        let data = base64::engine::general_purpose::STANDARD.decode(&chunk.data)?;
        anyhow::ensure!(bytes.len() + data.len() <= chunk.size, "Image size changed");
        bytes.extend(data);
        hash.get_or_insert(chunk.content_hash);
        mime = chunk.mime_type;
        if chunk.done {
            anyhow::ensure!(bytes.len() == chunk.size, "Image read was incomplete");
            break;
        }
        anyhow::ensure!(chunk.next_offset > offset, "Image read stopped");
        offset = chunk.next_offset;
    }
    Ok((bytes, mime))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poll_image<F: std::future::Future>(future: F) -> F::Output {
        use std::task::{Context, Poll, Waker};
        let mut future = Box::pin(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("fixture replies are immediately ready"),
        }
    }

    #[test]
    fn image_chunks_cannot_mix_checkout_or_content_versions() {
        for changed in ["checkoutId", "contentHash"] {
            let first = json!({"checkoutId":"checkout", "contentHash":"version", "size":2,
                "mimeType":"image/png", "data":"YQ==", "nextOffset":1, "done":false});
            let mut second = first.clone();
            second[changed] = json!("changed");
            second["done"] = json!(true);
            let replies = std::cell::RefCell::new(std::collections::VecDeque::from([
                json!({"checkoutId":"checkout"}),
                first,
                second,
            ]));
            let result = poll_image(read_image_bytes(
                |_, _| std::future::ready(Ok(replies.borrow_mut().pop_front().unwrap())),
                "chat",
                "image.png",
            ));
            assert!(result.is_err(), "accepted changed {changed}");
        }
    }

    #[test]
    fn closing_a_chip_keeps_a_shared_image_until_its_last_chip_closes() {
        let mut previews = Previews::default();
        for chip in ["first#g0#d0", "second#g0#d0"] {
            previews
                .chips
                .insert(chip.into(), vec!["/work/image.png".into()]);
        }
        previews.images.insert(
            "/work/image.png".into(),
            Cached {
                preview: Preview::Loading,
                bytes: 0,
                _task: None,
            },
        );
        previews.release_row("first#g0");
        assert_eq!(previews.images.len(), 1);
        previews.release_row("second#g0");
        assert!(previews.images.is_empty());
    }
}
