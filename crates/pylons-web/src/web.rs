//! The layer in the browser: web-sys over the page's elements.
//!
//! Every step returns `Option` rather than panicking; a step that can't
//! find what it needs leaves the page as the server sent it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use pylons_view::{Pylon, Shown, Snapshot, rows};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, EventSource, HtmlElement, KeyboardEvent, MessageEvent, Window};

use crate::camera::{Keep, Placed, blocked, spots};

/// The page's event stream.
const LIVE: &str = "/pylons/live";
/// How long a pillar takes to fade before it is removed, ms.
const LEAVE_MS: i32 = 900;
/// The corner name's bottom: tags stay below it, CSS px.
const TOP: f64 = 52.0;

struct Entry {
    pillar: HtmlElement,
    item: Element,
    view: Pylon,
    slot: usize,
    flip: bool,
}

struct Layer {
    window: Window,
    document: Document,
    field: Element,
    list: Element,
    count: Element,
    said: Element,
    card: Element,
    entries: BTreeMap<String, Entry>,
    /// Which pylon stands on each spot; `None` is a free spot.
    slots: Vec<Option<String>>,
    spots: Vec<Placed>,
    /// Boxes pylons stay out of: the list panel and anything the page marks
    /// `data-pylons-keep` (`/lander10`'s chat).
    keep: Vec<Keep>,
    /// Pillars fading out, by element, until they are removed.
    leaving: Vec<HtmlElement>,
    /// The site's clock minus this one, s.
    offset: f64,
    synced: bool,
    more: usize,
    seen: bool,
    open: Option<String>,
    opener: Option<HtmlElement>,
}

type Shared = Rc<RefCell<Layer>>;

/// Starts the layer: the page calls it once the module is ready.
#[wasm_bindgen]
pub fn start(still: bool) {
    let _ = still;
    let _ = run();
}

fn now_secs() -> f64 {
    js_sys::Date::now() / 1000.0
}

fn run() -> Option<()> {
    let window = web_sys::window()?;
    let document = window.document()?;
    let element = |id: &str| document.get_element_by_id(id);
    let layer = Rc::new(RefCell::new(Layer {
        field: element("pylons-field")?,
        list: element("pylons-list")?,
        count: element("pylons-count")?,
        said: element("pylons-said")?,
        card: element("pylon-card")?,
        window: window.clone(),
        document: document.clone(),
        entries: BTreeMap::new(),
        slots: Vec::new(),
        spots: Vec::new(),
        keep: Vec::new(),
        leaving: Vec::new(),
        offset: 0.0,
        synced: false,
        more: 0,
        seen: false,
        open: None,
        opener: None,
    }));
    layer.borrow_mut().layout();

    let source = EventSource::new(LIVE).ok()?;
    {
        let layer = layer.clone();
        let on_snapshot = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let Some(text) = event.data().as_string() else {
                return;
            };
            let Ok(snapshot) = serde_json::from_str::<Snapshot>(&text) else {
                return;
            };
            apply(&layer, snapshot);
        });
        source
            .add_event_listener_with_callback("pylons", on_snapshot.as_ref().unchecked_ref())
            .ok()?;
        on_snapshot.forget();
    }
    std::mem::forget(source);

    {
        let layer = layer.clone();
        let on_resize = Closure::<dyn FnMut()>::new(move || {
            let mut layer = layer.borrow_mut();
            layer.layout();
            layer.place_all();
        });
        window
            .add_event_listener_with_callback("resize", on_resize.as_ref().unchecked_ref())
            .ok()?;
        on_resize.forget();
    }

    // A page whose own content moves (the chat card growing) says so, and
    // the pillars make room.
    {
        let layer = layer.clone();
        let on_keep = Closure::<dyn FnMut()>::new(move || {
            let mut layer = layer.borrow_mut();
            layer.layout();
            layer.place_all();
        });
        document
            .add_event_listener_with_callback("pylons-keep", on_keep.as_ref().unchecked_ref())
            .ok()?;
        on_keep.forget();
    }

    // Clicks on a pillar or a list entry open its card; a click anywhere
    // else but the card closes it.
    {
        let layer = layer.clone();
        let on_click = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            let Some(target) = event.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
                return;
            };
            if target.closest("#pylon-close").ok().flatten().is_some() {
                close(&layer);
                return;
            }
            let Some(found) = target.closest("[data-pylon]").ok().flatten() else {
                let inside = target.closest("#pylon-card").ok().flatten().is_some();
                if !inside && layer.borrow().open.is_some() {
                    close(&layer);
                }
                return;
            };
            if let Some(id) = found.get_attribute("data-pylon") {
                let opener = target
                    .closest("button")
                    .ok()
                    .flatten()
                    .and_then(|b| b.dyn_into().ok());
                open(&layer, &id, opener);
            }
        });
        document
            .add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref())
            .ok()?;
        on_click.forget();
    }
    {
        let layer = layer.clone();
        let on_key = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
            if event.key() == "Escape" && layer.borrow().open.is_some() {
                close(&layer);
            }
        });
        document
            .add_event_listener_with_callback("keydown", on_key.as_ref().unchecked_ref())
            .ok()?;
        on_key.forget();
    }

    // Once a second: drop pylons whose beacons have expired by this clock
    // (the stream may have gone quiet), and age the open card's details.
    {
        let layer = layer.clone();
        let tick = Closure::<dyn FnMut()>::new(move || {
            let expired: Vec<String> = {
                let layer = layer.borrow();
                let now = now_secs() + layer.offset;
                layer
                    .entries
                    .iter()
                    .filter(|(_, entry)| now > entry.view.expires_at as f64 + 2.0)
                    .map(|(id, _)| id.clone())
                    .collect()
            };
            for id in expired {
                leave(&layer, &id, true);
            }
            layer.borrow().fill_facts();
        });
        window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                tick.as_ref().unchecked_ref(),
                1000,
            )
            .ok()?;
        tick.forget();
    }
    Some(())
}

/// A new snapshot: pylons arrive, change, and leave.
fn apply(layer: &Shared, snapshot: Snapshot) {
    let first = {
        let mut l = layer.borrow_mut();
        l.offset = snapshot.now as f64 - now_secs();
        l.synced = snapshot.synced;
        l.more = snapshot.more;
        let first = !l.seen;
        l.seen = true;
        if first {
            // The server's own list gives way to the live one.
            l.list.set_inner_html("");
        }
        first
    };
    let ids: Vec<String> = snapshot.pylons.iter().map(|p| p.id.clone()).collect();
    let gone: Vec<String> = layer
        .borrow()
        .entries
        .keys()
        .filter(|id| !ids.contains(id))
        .cloned()
        .collect();
    for id in gone {
        leave(layer, &id, true);
    }
    let mut arrived = Vec::new();
    for pylon in snapshot.pylons {
        let mut l = layer.borrow_mut();
        if l.entries.contains_key(&pylon.id) {
            l.update(pylon);
        } else {
            if !first {
                arrived.push(pylon.name.clone());
            }
            l.arrive(pylon);
        }
    }
    let l = layer.borrow();
    l.order_list(&ids);
    l.heading();
    if !arrived.is_empty() {
        let words = arrived
            .iter()
            .map(|name| format!("{name} is online."))
            .collect::<Vec<_>>()
            .join(" ");
        l.say(&words);
    }
    if let Some(id) = l.open.clone() {
        if let Some(entry) = l.entries.get(&id) {
            let shown = Shown {
                pylon: entry.view.clone(),
                now: (now_secs() + l.offset) as u64,
                gone: false,
            };
            l.show(&shown);
        }
    }
}

/// A pylon has gone offline, or its beacon expired: its pillar fades and
/// is removed, and an open card says so.
fn leave(layer: &Shared, id: &str, announce: bool) {
    let mut l = layer.borrow_mut();
    let Some(entry) = l.entries.remove(id) else {
        return;
    };
    if let Some(slot) = l.slots.get_mut(entry.slot) {
        *slot = None;
    }
    entry.item.remove();
    let _ = entry.pillar.class_list().remove_1("in");
    let _ = entry.pillar.class_list().add_1("out");
    let pillar = entry.pillar.clone();
    l.leaving.push(pillar.clone());
    let layer_for_removal = layer.clone();
    let remove = Closure::once_into_js(move || {
        pillar.remove();
        layer_for_removal
            .borrow_mut()
            .leaving
            .retain(|p| !p.is_same_node(Some(&pillar)));
    });
    let _ = l
        .window
        .set_timeout_with_callback_and_timeout_and_arguments_0(remove.unchecked_ref(), LEAVE_MS);
    if announce {
        l.say(&format!("{} went offline.", entry.view.name));
    }
    l.heading();
    if l.open.as_deref() == Some(id) {
        let shown = Shown {
            pylon: entry.view.clone(),
            now: (now_secs() + l.offset) as u64,
            gone: true,
        };
        l.show(&shown);
    }
}

fn open(layer: &Shared, id: &str, opener: Option<HtmlElement>) {
    let mut l = layer.borrow_mut();
    let Some(entry) = l.entries.get(id) else {
        return;
    };
    let shown = Shown {
        pylon: entry.view.clone(),
        now: (now_secs() + l.offset) as u64,
        gone: false,
    };
    if let Some(previous) = l.open.take()
        && let Some(entry) = l.entries.get(&previous)
    {
        let _ = entry.pillar.class_list().remove_1("sel");
    }
    if let Some(entry) = l.entries.get(id) {
        let _ = entry.pillar.class_list().add_1("sel");
    }
    l.open = Some(id.to_owned());
    if opener.is_some() {
        l.opener = opener;
    }
    l.show(&shown);
    if let Some(close) = l
        .document
        .get_element_by_id("pylon-close")
        .and_then(|e| e.dyn_into::<HtmlElement>().ok())
    {
        let _ = close.focus();
    }
}

fn close(layer: &Shared) {
    let mut l = layer.borrow_mut();
    if let Some(id) = l.open.take()
        && let Some(entry) = l.entries.get(&id)
    {
        let _ = entry.pillar.class_list().remove_1("sel");
    }
    call_card("hide", None);
    if let Some(opener) = l.opener.take() {
        let _ = opener.focus();
    }
}

/// `pylonsCard.show(json)` or `pylonsCard.hide()` on the page's loader.
fn call_card(method: &str, json: Option<&str>) {
    let card = js_sys::Reflect::get(&js_sys::global(), &"pylonsCard".into()).ok();
    let Some(card) = card.filter(|card| card.is_object()) else {
        return;
    };
    let Ok(function) = js_sys::Reflect::get(&card, &method.into()) else {
        return;
    };
    let Some(function) = function.dyn_ref::<js_sys::Function>() else {
        return;
    };
    let _ = match json {
        Some(json) => function.call1(&card, &json.into()),
        None => function.call0(&card),
    };
}

impl Layer {
    /// The spots for this view: enough for every slot, kept clear of the
    /// corner name and the list panel.
    fn layout(&mut self) {
        let width = self
            .window
            .inner_width()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(1280.0);
        let height = self
            .window
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(720.0);
        let mut keep: Vec<Keep> = Vec::new();
        let mut boxes: Vec<Element> = Vec::new();
        if let Some(panel) = self.document.get_element_by_id("pylons-panel") {
            boxes.push(panel);
        }
        if let Ok(marked) = self.document.query_selector_all("[data-pylons-keep]") {
            for i in 0..marked.length() {
                if let Some(element) = marked.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
                    boxes.push(element);
                }
            }
        }
        for element in boxes {
            let rect = element.get_bounding_client_rect();
            // A visually hidden list takes no room.
            if rect.width() < 2.0 || rect.height() < 2.0 {
                continue;
            }
            keep.push((
                rect.left() - 8.0,
                rect.top() - 8.0,
                rect.right() + 8.0,
                rect.bottom() + 8.0,
            ));
        }
        let count = self.slots.len().max(12);
        self.spots = spots(count, width, height, TOP, &keep);
        self.keep = keep;
    }

    fn place_all(&self) {
        for entry in self.entries.values() {
            self.place(entry);
        }
    }

    fn place(&self, entry: &Entry) {
        let Some(spot) = self.spots.get(entry.slot) else {
            // No room left on the floor (the page's own content covers it).
            let _ = entry.pillar.class_list().add_1("hid");
            return;
        };
        // A spot the page's own content now covers (the chat card grew
        // over it before the pillars moved) hides its pillar.
        let _ = entry
            .pillar
            .class_list()
            .toggle_with_force("hid", blocked(spot, &self.keep));
        let style = entry.pillar.style();
        let _ = style.set_property("--x", &format!("{:.1}px", spot.x));
        let _ = style.set_property("--y", &format!("{:.1}px", spot.y));
        let _ = style.set_property("--h", &format!("{:.1}px", spot.height));
        let _ = style.set_property("--bw", &format!("{:.2}px", spot.width));
        let _ = style.set_property("--hw", &format!("{:.1}px", (spot.width * 8.0).max(30.0)));
        let _ = style.set_property("--s", &format!("{:.3}", spot.scale));
        let _ = style.set_property(
            "z-index",
            &format!("{}", 10_000 - spot.depth.round() as i32),
        );
    }

    fn take_slot(&mut self, id: &str) -> usize {
        if let Some(free) = self.slots.iter().position(Option::is_none) {
            self.slots[free] = Some(id.to_owned());
            return free;
        }
        self.slots.push(Some(id.to_owned()));
        if self.slots.len() > self.spots.len() {
            self.layout();
        }
        self.slots.len() - 1
    }

    fn arrive(&mut self, pylon: Pylon) {
        let Some(pillar) = self
            .document
            .create_element("button")
            .ok()
            .and_then(|e| e.dyn_into::<HtmlElement>().ok())
        else {
            return;
        };
        let _ = pillar.set_attribute("type", "button");
        let _ = pillar.set_attribute("tabindex", "-1");
        let _ = pillar.set_attribute("data-pylon", &pylon.id);
        pillar.set_class_name("pylon");
        let item = match self.document.create_element("li") {
            Ok(item) => item,
            Err(_) => return,
        };
        let _ = item.set_attribute("data-id", &pylon.id);
        let slot = self.take_slot(&pylon.id);
        let entry = Entry {
            pillar,
            item,
            view: pylon,
            slot,
            flip: false,
        };
        self.draw(&entry);
        self.place(&entry);
        let _ = self.field.append_child(&entry.pillar);
        let _ = self.list.append_child(&entry.item);
        // Lay it out where it starts, then let it rise.
        let _ = entry.pillar.offset_width();
        let _ = entry.pillar.class_list().add_1("in");
        self.entries.insert(entry.view.id.clone(), entry);
    }

    fn update(&mut self, pylon: Pylon) {
        let Some(mut entry) = self.entries.remove(&pylon.id) else {
            return;
        };
        let before = entry.view.jobs.map(|jobs| jobs.receipts);
        let after = pylon.jobs.map(|jobs| jobs.receipts);
        let changed = entry.view != pylon;
        entry.view = pylon;
        if changed {
            self.draw(&entry);
        }
        if let (Some(before), Some(after)) = (before, after)
            && after > before
        {
            // A receipt arrived: restart the pulse.
            let classes = entry.pillar.class_list();
            let (on, off) = if entry.flip {
                ("pa", "pb")
            } else {
                ("pb", "pa")
            };
            let _ = classes.remove_1(off);
            let _ = classes.add_1(on);
            entry.flip = !entry.flip;
        }
        self.entries.insert(entry.view.id.clone(), entry);
    }

    /// The pillar's tag and the list entry, from the pylon's view.
    fn draw(&self, entry: &Entry) {
        let pylon = &entry.view;
        let draining = pylon.status == "draining";
        let _ = entry
            .pillar
            .class_list()
            .toggle_with_force("draining", draining);
        let slots: String = (0..pylon.total.min(16))
            .map(|i| {
                if i < pylon.busy {
                    "<i class=\"b\"></i>"
                } else {
                    "<i></i>"
                }
            })
            .collect();
        entry.pillar.set_inner_html(&format!(
            "<span class=\"ring\"></span><span class=\"base\"></span><span class=\"beam\"></span>\
<span class=\"tag\"><b>{}</b><small>{} · {}</small><span class=\"slots\">{slots}</span></span>",
            escape(&pylon.name),
            escape(pylon.class_words()),
            escape(&pylon.slots_words()),
        ));
        entry.item.set_inner_html(&format!(
            "<button type=\"button\" class=\"n\" data-pylon=\"{}\" aria-haspopup=\"dialog\">{}</button> \
<span class=\"d\">{}</span>",
            escape(&pylon.id),
            escape(&pylon.name),
            escape(&pylon.summary()),
        ));
    }

    /// Keeps the list in the snapshot's order.
    fn order_list(&self, ids: &[String]) {
        for id in ids {
            if let Some(entry) = self.entries.get(id) {
                let _ = self.list.append_child(&entry.item);
            }
        }
    }

    fn heading(&self) {
        let count = self.entries.len() + self.more;
        let words = match count {
            0 if !self.synced => "Looking for pylons".to_owned(),
            0 => "No pylons online".to_owned(),
            1 => "1 pylon online".to_owned(),
            n => format!("{n} pylons online"),
        };
        self.count.set_text_content(Some(&words));
    }

    fn say(&self, words: &str) {
        self.said.set_text_content(Some(words));
    }

    /// Hands the card a pylon, and fills the box's HTML details.
    fn show(&self, shown: &Shown) {
        if let Ok(json) = serde_json::to_string(shown) {
            call_card("show", Some(&json));
        }
        self.facts(shown);
    }

    /// The open card again, so its times stay current.
    fn fill_facts(&self) {
        let Some(id) = &self.open else {
            return;
        };
        if let Some(entry) = self.entries.get(id) {
            self.show(&Shown {
                pylon: entry.view.clone(),
                now: (now_secs() + self.offset) as u64,
                gone: false,
            });
        }
    }

    /// The card's details as HTML: what screen readers read, and what the
    /// box shows when GPUI can't start.
    fn facts(&self, shown: &Shown) {
        let Some(name) = self.document.get_element_by_id("pylon-facts-name") else {
            return;
        };
        name.set_text_content(Some(&shown.pylon.name));
        let Ok(Some(list)) = self.card.query_selector("#pylon-facts dl") else {
            return;
        };
        let status = if shown.gone {
            "Offline"
        } else {
            shown.pylon.status_words()
        };
        let mut html = format!(
            "<dt>Status</dt><dd>{status}</dd><dt>Key</dt><dd>{}</dd>",
            escape(&shown.pylon.npub)
        );
        for row in rows(shown) {
            html.push_str(&format!(
                "<dt>{}</dt><dd>{}</dd>",
                escape(row.label),
                escape(&row.value)
            ));
        }
        list.set_inner_html(&html);
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
