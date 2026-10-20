//! The computer chat's footer, popovers, and host watches, loaded separately
//! to keep the initial chat module within its transfer budget (#12270).
pub mod model;

#[cfg(target_family = "wasm")]
mod app;
#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(target_family = "wasm")]
thread_local! {
    static WINDOW: std::cell::RefCell<Option<(gpui::WeakEntity<app::Extras>, gpui::AsyncApp)>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(fonts: js_sys::Array, context: String) {
    use gpui::{AppContext, Application, WindowBounds, WindowOptions, px, size};
    use openagents_gpui_web::{FetchHttpClient, WebFonts, WebPlatform};
    use openagents_web_chat_view::{assets, theme};
    use std::{borrow::Cow, rc::Rc, sync::Arc};
    console_error_panic_hook::set_once();
    let platform = WebPlatform::new(
        false,
        WebFonts {
            fallback_family: theme::SANS,
            faces: fonts
                .iter()
                .map(|f| Cow::Owned(js_sys::Uint8Array::new(&f).to_vec()))
                .collect(),
        },
    );
    let handle = Application::with_platform(Rc::new(platform))
        .with_assets(assets::Assets)
        .with_http_client(Arc::new(FetchHttpClient::new()))
        .run_embedded(move |cx| {
            openagents_web_chat_view::install(cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_background: gpui::WindowBackgroundAppearance::Transparent,
                    window_bounds: Some(WindowBounds::Windowed(gpui::Bounds::centered(
                        None,
                        size(px(704.), px(76.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                move |_, cx| cx.new(|cx| app::Extras::new(&context, cx)),
            );
            if let Ok(entity) = opened.and_then(|w| w.entity(cx)) {
                WINDOW.with(|w| *w.borrow_mut() = Some((entity.downgrade(), cx.to_async())));
                openagents_web_client::store::signal("oa-extras-ready", "");
            }
            cx.activate(true);
        });
    std::mem::forget(handle);
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(context: String) {
    WINDOW.with(|w| {
        if let Some((entity, cx)) = w.borrow().as_ref() {
            let _ = entity.update(&mut cx.clone(), |extras, cx| extras.open(&context, cx));
        }
    });
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn fork() {
    WINDOW.with(|w| {
        if let Some((entity, cx)) = w.borrow().as_ref() {
            let _ = entity.update(&mut cx.clone(), |extras, cx| extras.fork(cx));
        }
    });
}

/// Tool bytes are assembled here so the initial chat doesn't link the chunk reader.
#[cfg(target_family = "wasm")]
#[wasm_bindgen(js_name = readImage)]
pub async fn read_image(device: String, chat: String, path: String) -> Result<String, JsValue> {
    use base64::Engine as _;
    use openagents_web_client::{computer, web::Client};
    let executor = WINDOW
        .with(|window| {
            window
                .borrow()
                .as_ref()
                .map(|(_, cx)| cx.background_executor().clone())
        })
        .ok_or_else(|| {
            JsValue::from_str("The image reader is starting. Close and reopen the tool to retry.")
        })?;
    let read =
        async {
            let host = computer::connect(&Client::from_page(), &device, || {}).await?;
            let (bytes, mime) = openagents_web_chat_view::read_image_bytes(
                |method, params| {
                    let host = host.clone();
                    async move { host.call(&method, params).await }
                },
                &chat,
                &path,
            )
            .await?;
            Ok::<_, anyhow::Error>(serde_json::json!({
            "data": base64::engine::general_purpose::STANDARD.encode(bytes), "mimeType": mime,
        }).to_string())
        };
    match futures::future::select(
        Box::pin(read),
        Box::pin(executor.timer(std::time::Duration::from_secs(60))),
    )
    .await
    {
        futures::future::Either::Left((result, _)) => {
            result.map_err(|error| JsValue::from_str(&error.to_string()))
        }
        futures::future::Either::Right(_) => Err(JsValue::from_str("Image read timed out")),
    }
}
