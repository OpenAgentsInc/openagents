use wasm_bindgen::prelude::JsValue;

pub(crate) fn js_optional_string(value: &JsValue) -> Option<String> {
    if value.is_null() || value.is_undefined() {
        None
    } else {
        value.as_string()
    }
}
