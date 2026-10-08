//! Keyed DOM reconciliation. Events remain the application's responsibility.

use crate::render;
use rust_native::ValidatedView;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    Element, HtmlDialogElement, HtmlElement, HtmlInputElement, HtmlTextAreaElement, Node,
};

/// Apply a view synchronously and acknowledge the displayed revision on the
/// mount root. A later callback reads that identity, not a queued revision.
///
/// The application delegates `click`, `input`, and capturing `cancel` events
/// from this root. During IME composition it defers field changes until commit.
/// This function installs no application callbacks and executes no intents.
pub fn mount<I>(root: &Element, view: &ValidatedView<I>) -> Result<(), JsValue> {
    let identity = view.view();
    let html = render(view).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let digest = digest(&html);
    let same_instance =
        root.get_attribute("data-rn-instance").as_deref() == Some(identity.instance.as_str());
    if same_instance {
        let applied = root
            .get_attribute("data-rn-revision")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        if identity.revision < applied {
            return Err(JsValue::from_str(
                "mount revision is older than the displayed view",
            ));
        }
        if identity.revision == applied {
            if root.get_attribute("data-rn-view-digest").as_deref() == Some(digest.as_str()) {
                return Ok(());
            }
            return Err(JsValue::from_str(
                "one view revision cannot identify different contents",
            ));
        }
    }
    let document = root
        .owner_document()
        .ok_or_else(|| JsValue::from_str("mount has no document"))?;
    let desired = document.create_element("div")?;
    // The only HTML input is this crate's escaped semantic renderer.
    desired.set_inner_html(&html);
    if !same_instance {
        retire(root.as_ref());
        root.set_text_content(None);
    }
    sync_children(root.as_ref(), desired.as_ref())?;
    initialize(root.as_ref())?;
    root.set_attribute("data-rn-instance", &identity.instance)?;
    root.set_attribute("data-rn-revision", &identity.revision.to_string())?;
    root.set_attribute("data-rn-view-digest", &digest)?;
    Ok(())
}

/// Retire a mount. The owner also removes its delegated listeners and drops
/// its view/controller, so events cannot keep acting on a disposed surface.
pub fn dispose(root: &Element) -> Result<(), JsValue> {
    retire(root.as_ref());
    for attr in [
        "data-rn-instance",
        "data-rn-revision",
        "data-rn-view-digest",
    ] {
        root.remove_attribute(attr)?;
    }
    root.set_text_content(None);
    Ok(())
}

fn digest(html: &str) -> String {
    // Detects accidental reuse of a revision; this is not authentication.
    let mut value = 0xcbf29ce484222325u64;
    for byte in html.bytes() {
        value ^= u64::from(byte);
        value = value.wrapping_mul(0x100000001b3);
    }
    format!("{value:016x}")
}
fn key(node: &Node) -> Option<String> {
    node.dyn_ref::<Element>()
        .and_then(|element| element.get_attribute("data-rn-node"))
}
fn compatible(old: &Node, new: &Node) -> bool {
    old.node_type() == new.node_type()
        && match (old.dyn_ref::<Element>(), new.dyn_ref::<Element>()) {
            (Some(old), Some(new)) => {
                old.tag_name() == new.tag_name()
                    && key(old.as_ref()) == key(new.as_ref())
                    && old.get_attribute("data-rn-token") == new.get_attribute("data-rn-token")
                    && old.get_attribute("data-rn-secret") == new.get_attribute("data-rn-secret")
                    && old.get_attribute("type") == new.get_attribute("type")
            }
            (None, None) => true,
            _ => false,
        }
}
fn sync_children(old: &Node, new: &Node) -> Result<(), JsValue> {
    let desired = new.child_nodes();
    for index in 0..desired.length() {
        let wanted = desired.item(index).expect("child index is present");
        let current = old.child_nodes().item(index);
        let candidate = if let Some(wanted_key) = key(&wanted) {
            let existing = old.child_nodes();
            (index..existing.length())
                .filter_map(|i| existing.item(i))
                .find(|node| key(node).as_deref() == Some(wanted_key.as_str()))
        } else {
            current.clone().filter(|node| compatible(node, &wanted))
        };
        match candidate {
            Some(candidate) if compatible(&candidate, &wanted) => {
                if !current
                    .as_ref()
                    .is_some_and(|current| current.is_same_node(Some(&candidate)))
                {
                    old.insert_before(&candidate, current.as_ref())?;
                }
                sync(&candidate, &wanted)?;
            }
            _ => {
                let inserted = wanted.clone_node_with_deep(true)?;
                old.insert_before(&inserted, current.as_ref())?;
            }
        }
    }
    while old.child_nodes().length() > desired.length() {
        if let Some(last) = old.last_child() {
            retire(&last);
            old.remove_child(&last)?;
        }
    }
    Ok(())
}
fn sync(old: &Node, new: &Node) -> Result<(), JsValue> {
    let (Some(old_element), Some(new_element)) =
        (old.dyn_ref::<Element>(), new.dyn_ref::<Element>())
    else {
        if old.node_value() != new.node_value() {
            old.set_node_value(new.node_value().as_deref());
        }
        return Ok(());
    };
    let input_state = input_state(old_element)?;
    let secret = old_element.get_attribute("data-rn-secret").as_deref() == Some("true");
    let top = old_element.scroll_top();
    let left = old_element.scroll_left();
    let transcript = old_element.class_list().contains("rn-transcript");
    let follow = transcript && old_element.scroll_height() - old_element.client_height() - top <= 2;
    let anchor = if transcript && !follow {
        anchor(old_element)
    } else {
        None
    };
    let attrs = old_element.attributes();
    let names: Vec<String> = (0..attrs.length())
        .filter_map(|index| attrs.item(index).map(|attr| attr.name()))
        .collect();
    for name in names {
        if name == "data-rn-modal"
            || name == "data-rn-focus-applied"
            || (name == "open" && matches!(old_element.tag_name().as_str(), "DIALOG" | "DETAILS"))
        {
            continue;
        }
        if !new_element.has_attribute(&name) {
            old_element.remove_attribute(&name)?;
        }
    }
    let attrs = new_element.attributes();
    for index in 0..attrs.length() {
        if let Some(attr) = attrs.item(index) {
            if attr.name() == "open" && old_element.tag_name() == "DIALOG" {
                continue;
            }
            if old_element.get_attribute(&attr.name()).as_deref() != Some(attr.value().as_str()) {
                old_element.set_attribute(&attr.name(), &attr.value())?;
            }
        }
    }
    if let Some(input) = old_element.dyn_ref::<HtmlInputElement>() {
        if let (Some(state), Some(new)) = (&input_state, new_element.dyn_ref::<HtmlInputElement>())
        {
            let rendered = new.value();
            let preserve =
                secret || old_element.get_attribute("data-rn-action").as_deref() == Some("compose");
            let value =
                crate::editing::next_value(&state.acknowledged, &state.live, &rendered, preserve);
            input.set_default_value(&rendered);
            if input.value() != value {
                input.set_value(value);
            }
            if value == state.live {
                if let (Some(start), Some(end)) = (state.start, state.end) {
                    let _ = input.set_selection_range_with_direction(start, end, &state.direction);
                }
            }
        }
    } else if let Some(input) = old_element.dyn_ref::<HtmlTextAreaElement>() {
        if let (Some(state), Some(new)) =
            (&input_state, new_element.dyn_ref::<HtmlTextAreaElement>())
        {
            let rendered = new.value();
            let preserve =
                secret || old_element.get_attribute("data-rn-action").as_deref() == Some("compose");
            let value =
                crate::editing::next_value(&state.acknowledged, &state.live, &rendered, preserve);
            input.set_default_value(&rendered)?;
            if input.value() != value {
                input.set_value(value);
            }
            if value == state.live {
                if let (Some(start), Some(end)) = (state.start, state.end) {
                    let _ = input.set_selection_range_with_direction(start, end, &state.direction);
                }
            }
        }
    } else {
        sync_children(old, new)?;
    }
    old_element.set_scroll_left(left);
    if follow {
        old_element.set_scroll_top(old_element.scroll_height());
    } else {
        old_element.set_scroll_top(top);
        if let Some((key, offset)) = anchor {
            if let Some(child) = old_element.query_selector(&format!("[data-rn-node=\"{key}\"]"))? {
                let new_offset = child.get_bounding_client_rect().top()
                    - old_element.get_bounding_client_rect().top();
                old_element.set_scroll_top(top + (new_offset - offset).round() as i32);
            }
        }
    }
    Ok(())
}

struct InputState {
    live: String,
    acknowledged: String,
    start: Option<u32>,
    end: Option<u32>,
    direction: String,
}
fn input_state(element: &Element) -> Result<Option<InputState>, JsValue> {
    if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
        Ok(Some(InputState {
            live: input.value(),
            acknowledged: input.default_value(),
            start: input.selection_start().ok().flatten(),
            end: input.selection_end().ok().flatten(),
            direction: input
                .selection_direction()
                .ok()
                .flatten()
                .unwrap_or_else(|| "none".into()),
        }))
    } else if let Some(input) = element.dyn_ref::<HtmlTextAreaElement>() {
        Ok(Some(InputState {
            live: input.value(),
            acknowledged: input.default_value()?,
            start: input.selection_start().ok().flatten(),
            end: input.selection_end().ok().flatten(),
            direction: input
                .selection_direction()
                .ok()
                .flatten()
                .unwrap_or_else(|| "none".into()),
        }))
    } else {
        Ok(None)
    }
}
fn anchor(element: &Element) -> Option<(String, f64)> {
    let top = element.get_bounding_client_rect().top();
    let mut child = element.first_element_child();
    while let Some(row) = child {
        let rect = row.get_bounding_client_rect();
        if rect.bottom() > top {
            if let Some(key) = row.get_attribute("data-rn-node") {
                return Some((key, rect.top() - top));
            }
        }
        child = row.next_element_sibling();
    }
    None
}
fn initialize(node: &Node) -> Result<(), JsValue> {
    if let Some(element) = node.dyn_ref::<Element>() {
        let children = element.child_nodes();
        for index in 0..children.length() {
            if let Some(child) = children.item(index) {
                initialize(&child)?;
            }
        }
        dialog(element)?;
        if element.get_attribute("data-rn-focus").as_deref() == Some("true")
            && !element.has_attribute("data-rn-focus-applied")
            && !element.has_attribute("disabled")
        {
            if let Some(element) = element.dyn_ref::<HtmlElement>() {
                element.focus()?;
                element.set_attribute("data-rn-focus-applied", "true")?;
            }
        }
    }
    Ok(())
}

fn retire(node: &Node) {
    let children = node.child_nodes();
    for index in 0..children.length() {
        if let Some(child) = children.item(index) {
            retire(&child);
        }
    }
    if let Some(dialog) = node.dyn_ref::<HtmlDialogElement>() {
        if dialog.open() {
            dialog.close();
        }
    }
    // Detached controls can survive in a late event or a caller's reference.
    // Retire their input contents before removing their DOM lifetime.
    if let Some(input) = node.dyn_ref::<HtmlInputElement>() {
        input.set_value("");
        let _ = input.remove_attribute("value");
    }
    if let Some(input) = node.dyn_ref::<HtmlTextAreaElement>() {
        input.set_value("");
        input.set_text_content(None);
    }
}
fn dialog(element: &Element) -> Result<(), JsValue> {
    if let Some(dialog) = element.dyn_ref::<HtmlDialogElement>() {
        let should_open = element.get_attribute("data-rn-dialog").as_deref() == Some("true");
        if should_open && element.get_attribute("data-rn-modal").as_deref() != Some("true") {
            if dialog.open() {
                dialog.close();
            }
            dialog.show_modal()?;
            element.set_attribute("data-rn-modal", "true")?;
        } else if !should_open {
            if dialog.open() {
                dialog.close();
            }
            element.remove_attribute("data-rn-modal")?;
        }
    }
    Ok(())
}
