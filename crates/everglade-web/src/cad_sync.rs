//! Account-authenticated CAD physics polling. The host owns body simulation.
use crate::web::{Page, fetch_cad_models};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use verse::zones::everglade::cad_physics::sync::Snapshot;
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{RequestCredentials, RequestInit, RequestRedirect, Response, Window};
const API: &str = "/cad-world/physics";
async fn fetch(window: &Window, path: &str, body: Option<&str>) -> Result<(u16, String), String> {
    let init = RequestInit::new();
    init.set_credentials(RequestCredentials::SameOrigin);
    init.set_redirect(RequestRedirect::Error);
    let headers = web_sys::Headers::new().map_err(|_| "The world could not be opened.")?;
    if let Some(body) = body {
        init.set_method("POST");
        init.set_body(&JsValue::from_str(body));
        headers
            .set("Content-Type", "application/json")
            .map_err(|_| "The world could not be opened.")?;
    }
    init.set_headers(&headers);
    let response: Response = JsFuture::from(window.fetch_with_str_and_init(path, &init))
        .await
        .map_err(|_| "The world could not connect.")?
        .dyn_into()
        .map_err(|_| "The world could not connect.")?;
    let status = response.status();
    let text = JsFuture::from(
        response
            .text()
            .map_err(|_| "The world could not be read.")?,
    )
    .await
    .map_err(|_| "The world could not be read.")?
    .as_string()
    .ok_or("The world could not be read.")?;
    if text.len() > 4 << 20 {
        return Err("This world's movement is too large.".into());
    }
    Ok((status, text))
}
fn report(window: &Window, page: &Page, revision: u64) {
    if !window
        .location()
        .search()
        .unwrap_or_default()
        .contains("frames")
    {
        return;
    }
    if let Some(canvas) = window
        .document()
        .and_then(|d| d.get_element_by_id("everglade-canvas"))
    {
        let reports: Vec<_> = page.runtime.cad_placed().iter().map(|p| {
            let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
            for vertex in &p.vertices {
                let at = glam::Vec3::from_array(vertex.pos);
                lo = lo.min(at);
                hi = hi.max(at);
            }
            serde_json::json!({"id":p.placement.key(),"version":p.placement.version,"vertices":p.vertices.len(),"bounds":[lo,hi],"state":page.runtime.cad_report(p.placement.key())})
        }).collect();
        let _ = canvas.set_attribute(
            "data-cad-shared",
            &serde_json::json!({"revision":revision,"models":reports}).to_string(),
        );
    }
}
pub(crate) fn start(window: Window, page: Rc<RefCell<Page>>) {
    if !matches!(
        page.borrow().runtime.zone.world_id(),
        "verse-everglade" | "verse-meteor-showcase" | "verse-meteor-stress-test"
    ) {
        return;
    }
    let mut random = [0; 16];
    if window
        .crypto()
        .and_then(|c| c.get_random_values_with_u8_array(&mut random))
        .is_err()
    {
        return;
    }
    let client: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let busy = Rc::new(Cell::new(false));
    let enabled = Rc::new(Cell::new(true));
    let initialized = Rc::new(Cell::new(false));
    let sequence = Rc::new(Cell::new(1u64));
    let revision = Rc::new(Cell::new(0u64));
    let pending: Rc<RefCell<Option<String>>> = Rc::default();
    let due = Rc::new(Cell::new(0.));
    let poll = Closure::<dyn FnMut()>::new({
        let window = window.clone();
        move || {
            let time = js_sys::Date::now();
            if !enabled.get() || busy.get() || time < due.get() {
                return;
            }
            due.set(
                time + if page.borrow().runtime.cad_placed().is_empty() {
                    2000.
                } else {
                    200.
                },
            );
            if busy.replace(true) {
                return;
            }
            let (window, page, busy, enabled, initialized, sequence, pending, client, revision) = (
                window.clone(),
                page.clone(),
                busy.clone(),
                enabled.clone(),
                initialized.clone(),
                sequence.clone(),
                pending.clone(),
                client.clone(),
                revision.clone(),
            );
            spawn_local(async move {
                let zone = page.borrow().runtime.zone.world_id().to_owned();
                if !matches!(
                    zone.as_str(),
                    "verse-everglade" | "verse-meteor-showcase" | "verse-meteor-stress-test"
                ) {
                    enabled.set(false);
                    busy.set(false);
                    return;
                }
                let result:Result<(),String>=async {
                if !initialized.get() {
                    let (status,text)=fetch(&window,&format!("{API}?zone={zone}"),None).await?;
                    if status==401 || status==404 {enabled.set(false);page.borrow_mut().runtime.install_cad_models(Vec::new())?;return Ok(());}
                    if status!=200 {return Err("Your shared world could not be opened.".into());}
                    let snapshot:Snapshot=serde_json::from_str(&text).map_err(|_|"Your shared world could not be read.")?;
                    page.borrow_mut().runtime.enable_cad_sync(snapshot.account.clone());
                    if page.borrow_mut().runtime.apply_cad_snapshot(&snapshot).is_err() {
                        let models=fetch_cad_models(&window,&zone).await?;
                        page.borrow_mut().runtime.install_cad_models(models)?;
                        page.borrow_mut().runtime.apply_cad_snapshot(&snapshot)?;
                    }
                    revision.set(snapshot.revision);initialized.set(true);
                }
                if pending.borrow().is_none() {
                    let mut page=page.borrow_mut();
                    let bindings=page.runtime.cad_bindings();
                    let mut inputs=page.runtime.take_cad_inputs().unwrap_or_default();
                    if window.document().is_some_and(|d|d.hidden()) {inputs.forces.push=glam::Vec3::ZERO;}
                    *pending.borrow_mut()=Some(serde_json::json!({"zone":zone,"client":client,"sequence":sequence.get(),"base":revision.get(),"bindings":bindings,"inputs":inputs}).to_string());
                }
                let body=pending.borrow().clone().unwrap();
                let (status,text)=fetch(&window,API,Some(&body)).await?;
                if status==409 {
                    // Replacement and removal are admitted through the same private model list.
                    let models=fetch_cad_models(&window,&zone).await?;
                    page.borrow_mut().runtime.install_cad_models(models)?;
                    *pending.borrow_mut()=None;sequence.set(sequence.get().saturating_add(1));initialized.set(false);return Ok(());
                }
                if status==401 {enabled.set(false);page.borrow_mut().runtime.install_cad_models(Vec::new())?;return Err("Sign in again to reconnect your world.".into());}
                if status==400 {*pending.borrow_mut()=None;sequence.set(sequence.get().saturating_add(1));initialized.set(false);return Err("One action could not be shared.".into());}
                if status!=200 {return Err("Your world could not save movement. Reconnecting…".into());}
                let snapshot:Snapshot=serde_json::from_str(&text).map_err(|_|"Your shared world could not be read.")?;
                page.borrow_mut().runtime.apply_cad_snapshot(&snapshot)?;report(&window,&page.borrow(),snapshot.revision);revision.set(snapshot.revision);
                *pending.borrow_mut()=None;sequence.set(sequence.get().saturating_add(1));
                Ok(())
            }.await;
                if let Err(error) = result {
                    web_sys::console::warn_1(&error.into());
                }
                busy.set(false);
            });
        }
    });
    let function = poll.into_js_value();
    let _ = window
        .set_interval_with_callback_and_timeout_and_arguments_0(function.unchecked_ref(), 200);
}
