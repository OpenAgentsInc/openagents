use futures::channel::mpsc::UnboundedSender;
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::JsFuture;
use web_sys::{File, HtmlInputElement};

pub fn intake(tx: UnboundedSender<File>) {
    let document = web_sys::window().unwrap().document().unwrap();
    for kind in ["paste", "drop", "dragover"] {
        let tx = tx.clone();
        let callback = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            let data = event
                .dyn_ref::<web_sys::ClipboardEvent>()
                .and_then(|e| e.clipboard_data())
                .or_else(|| {
                    event
                        .dyn_ref::<web_sys::DragEvent>()
                        .and_then(|e| e.data_transfer())
                });
            if let Some(data) = data {
                if let Some(files) = data.files() {
                    if files.length() > 0 || kind == "dragover" {
                        event.prevent_default();
                    }
                    if kind != "dragover" {
                        for index in 0..files.length() {
                            if let Some(file) = files.get(index) {
                                let _ = tx.unbounded_send(file);
                            }
                        }
                    }
                }
            }
        });
        document
            .add_event_listener_with_callback(kind, callback.as_ref().unchecked_ref())
            .unwrap();
        callback.forget();
    }
}

pub fn pick(tx: UnboundedSender<File>) {
    let document = web_sys::window().unwrap().document().unwrap();
    let input: HtmlInputElement = document.create_element("input").unwrap().unchecked_into();
    input.set_type("file");
    input.set_multiple(true);
    let selected = input.clone();
    let callback = Closure::once(move |_: web_sys::Event| {
        if let Some(files) = selected.files() {
            for index in 0..files.length() {
                if let Some(file) = files.get(index) {
                    let _ = tx.unbounded_send(file);
                }
            }
        }
        selected.remove();
    });
    input.set_onchange(Some(callback.as_ref().unchecked_ref()));
    input.click();
    callback.forget();
}

pub async fn screenshot() -> Result<File, JsValue> {
    let window = web_sys::window().unwrap();
    let devices = window.navigator().media_devices()?;
    let stream: web_sys::MediaStream = JsFuture::from(devices.get_display_media()?)
        .await?
        .dyn_into()?;
    let result = async {
        let document = window.document().unwrap();
        let video: web_sys::HtmlVideoElement = document.create_element("video")?.dyn_into()?;
        video.set_muted(true);
        video.set_src_object(Some(&stream));
        JsFuture::from(video.play()?).await?;
        let canvas: web_sys::HtmlCanvasElement = document.create_element("canvas")?.dyn_into()?;
        canvas.set_width(video.video_width());
        canvas.set_height(video.video_height());
        let context: web_sys::CanvasRenderingContext2d =
            canvas.get_context("2d")?.unwrap().dyn_into()?;
        context.draw_image_with_html_video_element(&video, 0., 0.)?;
        let url = canvas.to_data_url_with_type("image/png")?;
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(url.split_once(',').unwrap().1)
            .map_err(|_| JsValue::from_str("Could not capture this screen."))?;
        let parts = js_sys::Array::new();
        parts.push(&js_sys::Uint8Array::from(bytes.as_slice()));
        File::new_with_u8_array_sequence(&parts, "screenshot.png")
    }
    .await;
    for track in stream.get_tracks().iter() {
        track.unchecked_into::<web_sys::MediaStreamTrack>().stop();
    }
    result
}

pub fn preview_url(file: &File) -> String {
    web_sys::Url::create_object_url_with_blob(file).unwrap_or_default()
}

pub fn preview(url: &str) {
    let document = web_sys::window().unwrap().document().unwrap();
    let dialog: web_sys::HtmlDialogElement =
        document.create_element("dialog").unwrap().unchecked_into();
    let image = document.create_element("img").unwrap();
    let _ = image.set_attribute("src", url);
    let _ = image.set_attribute("style", "max-width:90vw;max-height:85vh");
    let _ = image.set_attribute("alt", "Attachment preview");
    let _ = dialog.append_child(&image);
    let _ = document.body().unwrap().append_child(&dialog);
    let close = dialog.clone();
    let callback = Closure::once(move |_: web_sys::Event| close.remove());
    dialog.set_onclick(Some(callback.as_ref().unchecked_ref()));
    let _ = dialog.show_modal();
    callback.forget();
}
