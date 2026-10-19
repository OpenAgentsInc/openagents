use super::*;
use crate::layout::{TranscriptView, display::WidgetKind};
const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;

enum Source {
    Live {
        client: Arc<CoreClient>,
        device: String,
    },
    Saved {
        history: Arc<CoreSavedHistory>,
        chat: String,
    },
}
fn reference(
    state: &State,
    view: &TranscriptView,
    generation: u64,
    content: u64,
    revision: u64,
    row: u64,
    widget: u32,
) -> CoreResult<String> {
    let frame = view.frame();
    if state.generation != generation
        || state.content_revision != content
        || !state.foreground
        || state.route != AndroidHostRoute::Chat
        || frame.revision() != revision
    {
        return Err(CoreError::Closed);
    }
    let display = frame
        .index_of(row)
        .and_then(|i| frame.display(i))
        .ok_or(CoreError::Closed)?;
    match display
        .widgets
        .into_iter()
        .find(|item| item.id == widget)
        .map(|w| w.kind)
    {
        Some(WidgetKind::Image { reference }) => Ok(reference),
        _ => Err(CoreError::Closed),
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) async fn load(
    owner: &CoreAndroidHost,
    view: Arc<TranscriptView>,
    generation: u64,
    content: u64,
    revision: u64,
    row: u64,
    widget: u32,
) -> CoreResult<Option<Vec<u8>>> {
    let (path, source) = {
        let mut state = owner.state.lock().map_err(|_| CoreError::Closed)?;
        owner.refresh(&mut state)?;
        let path = reference(&state, &view, generation, content, revision, row, widget)?;
        let source = match &state.content {
            Content::Live { client, id, .. } => Source::Live {
                device: client
                    .session_row(id.clone())
                    .ok_or(CoreError::Closed)?
                    .device_id,
                client: client.clone(),
            },
            Content::Saved { history, id } => {
                if history
                    .reader()
                    .target()
                    .is_none_or(|target| target.chat.id() != id.as_str())
                {
                    return Err(CoreError::Closed);
                }
                Source::Saved {
                    history: history.clone(),
                    chat: id.clone(),
                }
            }
            // Neither text-only source provides an attachment contract.
            Content::Cloud(_) | Content::Indexed { .. } => return Ok(None),
        };
        (path, source)
    };
    let mut bytes = match &source {
        Source::Live { client, device } => {
            client.read_attachment(device.clone(), path.clone()).await?
        }
        Source::Saved { history, .. } => history.attachment_bytes(path.clone()).await?,
    };
    let checked = (|| {
        let mut state = owner.state.lock().map_err(|_| CoreError::Closed)?;
        owner.refresh(&mut state)?;
        if reference(&state, &view, generation, content, revision, row, widget)? != path {
            return Err(CoreError::Closed);
        }
        if let Source::Saved { history, chat } = &source {
            if !history.current_view()
                || history
                    .reader()
                    .target()
                    .is_none_or(|target| target.chat.id() != chat.as_str())
            {
                return Err(CoreError::Closed);
            }
        }
        Ok(())
    })();
    if let Err(error) = checked {
        bytes.fill(0);
        return Err(error);
    }
    if bytes.len() > MAX_IMAGE_BYTES {
        bytes.fill(0);
        return Ok(None);
    }
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct OwnerListener;
    impl AndroidHostListener for OwnerListener {
        fn on_android_host_changed(&self) {}
    }
    struct Frames(std::sync::mpsc::Sender<()>);
    impl crate::layout::LayoutListener for Frames {
        fn frame_ready(&self, _: u64) {
            let _ = self.0.send(());
        }
    }
    fn fixture() -> (Arc<CoreAndroidHost>, Arc<TranscriptView>, u64, u64, u32) {
        let owner = CoreAndroidHost::demo(Arc::new(OwnerListener));
        let (send, receive) = std::sync::mpsc::channel();
        let view = TranscriptView::new(crate::layout::tests::text_system(), Arc::new(Frames(send)));
        view.set_viewport(390.0, 1.0);
        view.set_debug_entries(vec![crate::layout::DebugEntry {
            id: "synthetic-image".into(), user: true, streaming: false,
            text: "Image fixture\n\nAttached images (local files — open them to view):\n- /synthetic/image.png".into(),
        }], false);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let frame = view.frame();
            for index in 0..frame.row_count() {
                let display = frame.display(index).unwrap();
                if let Some(widget) = display
                    .widgets
                    .iter()
                    .find(|w| matches!(w.kind, WidgetKind::Image { .. }))
                {
                    return (
                        owner,
                        view.clone(),
                        frame.revision(),
                        display.key,
                        widget.id,
                    );
                }
            }
            assert!(
                std::time::Instant::now() < deadline,
                "synthetic image layout timed out"
            );
            let _ = receive.recv_timeout(std::time::Duration::from_millis(50));
        }
    }
    #[test]
    fn image_reference_requires_exact_account_content_frame_row_and_widget() {
        let (owner, view, revision, row, widget) = fixture();
        let mut state = owner.state.lock().unwrap();
        let generation = state.generation;
        let content = state.content_revision;
        assert_eq!(
            reference(&state, &view, generation, content, revision, row, widget).unwrap(),
            "/synthetic/image.png"
        );
        for (generation, content, revision, row, widget) in [
            (generation + 1, content, revision, row, widget),
            (generation, content + 1, revision, row, widget),
            (generation, content, revision + 1, row, widget),
            (generation, content, revision, u64::MAX, widget),
            (generation, content, revision, row, u32::MAX),
        ] {
            assert!(reference(&state, &view, generation, content, revision, row, widget).is_err());
        }
        state.foreground = false;
        assert!(reference(&state, &view, generation, content, revision, row, widget).is_err());
        drop(state);
        view.close();
    }
    #[test]
    fn cloud_images_cannot_fall_through_to_a_legacy_saved_reader() {
        let (owner, view, revision, row, widget) = fixture();
        let snapshot = owner.snapshot().unwrap();
        let result = openagents_client::runtime::shared()
            .block_on(load(
                &owner,
                view.clone(),
                snapshot.generation,
                snapshot.content_revision,
                revision,
                row,
                widget,
            ))
            .unwrap();
        assert!(result.is_none());
        view.close();
    }
}
