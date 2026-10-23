//! Rust entry controls and destination state over the private universe profile.
use crate::*;
use js_sys::Promise;
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Event, EventTarget, HtmlDialogElement, HtmlElement};
type Handler = Closure<dyn FnMut(Event)>;
const SAVED: &str = "openagents.verse-map.v1";
fn window() -> web_sys::Window {
    web_sys::window().expect("browser")
}
fn element(id: &str) -> web_sys::Element {
    window()
        .document()
        .expect("document")
        .get_element_by_id(id)
        .expect("map element")
}
fn hidden(id: &str, yes: bool) {
    if yes {
        let _ = element(id).set_attribute("hidden", "");
    } else {
        let _ = element(id).remove_attribute("hidden");
    }
}
struct State {
    demo: BrowserDemo,
    visit: UniverseVisit,
    desired: Option<String>,
    restore_sent: bool,
    popup: Option<web_sys::Window>,
}
impl State {
    fn save(&self) {
        if let Ok(Some(storage)) = window().session_storage() {
            let _ = storage.set_item(SAVED, &serde_json::to_string(&self.visit).unwrap());
        }
    }
    fn render(&self) {
        let entered = self.visit.choice.is_some();
        hidden("verse-account", entered);
        hidden("verse-choices", !entered);
        let exploration = element("universe-exploration");
        if self.visit.exploring {
            let _ = exploration.remove_attribute("inert");
        } else {
            let _ = exploration.set_attribute("inert", "");
        }
        let card = element("verse-entry")
            .dyn_into::<HtmlDialogElement>()
            .unwrap();
        let _ = card.set_attribute(
            "aria-labelledby",
            if entered {
                "choice-title"
            } else {
                "entry-title"
            },
        );
        let _ = card.set_attribute(
            "aria-describedby",
            if entered {
                "choice-description"
            } else {
                "entry-description"
            },
        );
        if self.visit.exploring {
            card.close();
        } else if !card.open() {
            let _ = card.show_modal();
        }
        let _ = element("verse-atlas")
            .set_attribute("data-entry", if entered { "guest" } else { "locked" });
        if let Some(world) = WORLDS.iter().find(|w| w.id == self.visit.selected) {
            element("world-title").set_text_content(Some(world.name));
            element("world-category").set_text_content(Some(world.category));
            element("world-description").set_text_content(Some(world.description));
            for world in WORLDS {
                hidden(
                    &format!("destination-{}", world.id),
                    world.id != self.visit.selected,
                );
                let _ = element(&format!("option-{}", world.id)).set_attribute(
                    "aria-pressed",
                    if world.id == self.visit.selected {
                        "true"
                    } else {
                        "false"
                    },
                );
            }
        }
    }
    fn select(&mut self, id: &str) {
        if self.visit.select(id).is_err() {
            return;
        }
        self.desired = Some(id.into());
        if let Some(host) = self.demo.host.borrow().as_ref() {
            let _ =
                host.request(serde_json::to_string(&Action::Select { target: id.into() }).unwrap());
        }
        self.save();
        self.render();
    }
}
#[wasm_bindgen]
pub struct BrowserUniverse {
    state: Rc<RefCell<State>>,
    listeners: Vec<(EventTarget, &'static str, Handler)>,
    timer: i32,
    tick: Closure<dyn FnMut()>,
}
#[wasm_bindgen]
impl BrowserUniverse {
    pub fn snapshot(&self) -> String {
        self.state.borrow().demo.snapshot()
    }
    pub fn visit(&self) -> String {
        serde_json::to_string(&self.state.borrow().visit).unwrap()
    }
    pub fn close(&self) {
        self.state.borrow().demo.close();
    }
}
impl Drop for BrowserUniverse {
    fn drop(&mut self) {
        window().clear_interval_with_handle(self.timer);
        for (target, event, handler) in &self.listeners {
            let _ =
                target.remove_event_listener_with_callback(event, handler.as_ref().unchecked_ref());
        }
        self.close();
        let _ = &self.tick;
    }
}
fn listen(
    listeners: &mut Vec<(EventTarget, &'static str, Handler)>,
    id: &str,
    event: &'static str,
    mut action: impl FnMut(Event) + 'static,
) -> Result<(), JsValue> {
    let target: EventTarget = element(id).into();
    let handler = Closure::wrap(Box::new(move |event| action(event)) as Box<dyn FnMut(Event)>);
    target.add_event_listener_with_callback(event, handler.as_ref().unchecked_ref())?;
    listeners.push((target, event, handler));
    Ok(())
}
async fn pause() {
    let p = Promise::new(&mut |resolve, _| {
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 2000);
    });
    let _ = JsFuture::from(p).await;
}
#[wasm_bindgen]
pub fn mount_universe() -> Result<BrowserUniverse, JsValue> {
    let mut visit = window()
        .session_storage()?
        .and_then(|s| s.get_item(SAVED).ok().flatten())
        .and_then(|s| serde_json::from_str::<UniverseVisit>(&s).ok())
        .filter(|v| v.validate().is_ok())
        .unwrap_or_default();
    if element("verse-atlas")
        .get_attribute("data-entry")
        .as_deref()
        == Some("member")
    {
        visit.enter(EntryChoice::Member);
    }
    let demo = mount_demo()?;
    if let Some(c) = &visit.checkpoint {
        if let Some(host) = demo.host.borrow().as_ref() {
            host.restore_checkpoint(serde_json::to_string(c).unwrap())?;
        }
    }
    let rc = Rc::new(RefCell::new(State {
        demo,
        visit,
        desired: None,
        restore_sent: false,
        popup: None,
    }));
    rc.borrow().render();
    let mut listeners = Vec::new();
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "continue-guest", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            let mut s = rc.borrow_mut();
            s.visit.enter(EntryChoice::Guest);
            s.save();
            s.render();
            let _ = element("verse-explore")
                .dyn_into::<HtmlElement>()
                .unwrap()
                .focus();
        }
    })?;
    let weak = Rc::downgrade(&rc);
    listen(&mut listeners, "verse-explore", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            let mut s = rc.borrow_mut();
            if s.visit.explore().is_ok() {
                s.save();
                s.render();
                let _ = element("option-openagents")
                    .dyn_into::<HtmlElement>()
                    .unwrap()
                    .focus();
            }
        }
    })?;
    listen(&mut listeners, "verse-entry", "cancel", |event| {
        event.prevent_default()
    })?;
    for world in WORLDS {
        let weak = Rc::downgrade(&rc);
        listen(
            &mut listeners,
            &format!("option-{}", world.id),
            "click",
            move |_| {
                if let Some(rc) = weak.upgrade() {
                    rc.borrow_mut().select(world.id);
                }
            },
        )?;
    }
    let weak = Rc::downgrade(&rc);
    listen(
        &mut listeners,
        "verse-signin",
        "click",
        move |event| match window().open_with_url_and_target_and_features(
            "/auth/popup",
            "oa-verse-sign-in",
            "popup,width=560,height=760",
        ) {
            Ok(Some(popup)) => {
                event.prevent_default();
                if let Some(rc) = weak.upgrade() {
                    rc.borrow_mut().popup = Some(popup);
                }
            }
            _ => (),
        },
    )?;
    let weak = Rc::downgrade(&rc);
    let tick = Closure::wrap(Box::new(move || {
        let Some(rc) = weak.upgrade() else { return };
        let mut s = rc.borrow_mut();
        let value: Value = serde_json::from_str(&s.demo.snapshot()).unwrap_or(Value::Null);
        if value["phase"] == "open" {
            if !s.restore_sent {
                s.restore_sent = true;
                if s.visit.exploring {
                    let id = s.visit.selected.clone();
                    s.select(&id);
                }
            }
            if let Ok(c) =
                serde_json::from_value::<Checkpoint>(value["status"]["checkpoint"].clone())
            {
                if c.validate().is_ok() {
                    s.visit.checkpoint = Some(c);
                }
            }
            if let Some(id) = value["status"]["selection"]["id"].as_str() {
                if s.desired.as_deref() == Some(id) {
                    s.desired = None;
                }
                if s.desired.is_none() && s.visit.exploring && s.visit.selected != id {
                    if s.visit.select(id).is_ok() {
                        s.render();
                    }
                }
            }
            s.save();
        }
    }) as Box<dyn FnMut()>);
    let timer = window().set_interval_with_callback_and_timeout_and_arguments_0(
        tick.as_ref().unchecked_ref(),
        200,
    )?;
    let weak = Rc::downgrade(&rc);
    spawn_local(async move {
        loop {
            pause().await;
            let Some(rc) = weak.upgrade() else { break };
            if rc.borrow().visit.choice.is_some() {
                continue;
            }
            let promise = window().fetch_with_str("/v1/verse/studio/session");
            let Ok(response) = JsFuture::from(promise).await else {
                continue;
            };
            let Ok(response) = response.dyn_into::<web_sys::Response>() else {
                continue;
            };
            if !response.ok() {
                continue;
            }
            let Ok(json) = response.json() else { continue };
            let Ok(value) = JsFuture::from(json).await else {
                continue;
            };
            let Ok(text) = js_sys::JSON::stringify(&value) else {
                continue;
            };
            let value: Value =
                serde_json::from_str(&text.as_string().unwrap_or_default()).unwrap_or(Value::Null);
            if value["signed_in"] == true {
                let mut s = rc.borrow_mut();
                s.visit.enter(EntryChoice::Member);
                s.save();
                s.render();
            }
        }
    });
    Ok(BrowserUniverse {
        state: rc,
        listeners,
        timer,
        tick,
    })
}
