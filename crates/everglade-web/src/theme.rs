//! Application chrome tokens; world materials retain their owning renderer's colors.

use wasm_bindgen::JsValue;
use web_sys::Document;

pub fn linear(value: u32, alpha: f32) -> [f32; 4] {
    let [r, g, b] = verse::palette::linear(value);
    [r, g, b, alpha]
}

pub fn install(document: &Document) -> Result<(), JsValue> {
    let style = document
        .get_element_by_id("everglade-application-theme")
        .map_or_else(|| document.create_element("style"), Ok)?;
    style.set_id("everglade-application-theme");
    style.set_text_content(Some(&format!(
        "{}#host-terminal-controls button,#host-terminal-controls textarea,#chamber-controls button{{background:var(--noir-surface);color:var(--noir-content);border-color:var(--noir-stroke);caret-color:var(--noir-cursor)}}#host-terminal-controls button:focus-visible,#chamber-controls button:focus-visible{{outline-color:var(--noir-accent)}}",
        coder_ui::coder_noir::css_variables(),
    )));
    if !style.is_connected() {
        document
            .document_element()
            .ok_or(JsValue::NULL)?
            .append_child(&style)?;
    }
    Ok(())
}
