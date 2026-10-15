//! Share files, short codes, and the replay-checked public gallery.
use crate::hud::{css, element};
use snap_rules::{
    Input,
    receipt::{self, Receipt},
};
use snap_share::Shared;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{HtmlElement, HtmlInputElement, Request, RequestInit, Response};

const API: &str = "/games/snap-station/shares";
const KEY: &str = "snap.sharing-key.v1";
type Imports = Rc<RefCell<Vec<(usize, Vec<Input>)>>>;
#[derive(Clone)]
pub struct Sharing {
    root: HtmlElement,
    face: HtmlElement,
    imports: Imports,
}
fn click(button: &HtmlElement, f: impl FnMut() + 'static) {
    let mut f = f;
    let callback = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| f());
    let function = callback.into_js_value();
    let _ = button.add_event_listener_with_callback("click", function.unchecked_ref());
}
fn button(parent: &HtmlElement, label: &str) -> HtmlElement {
    let doc = parent.owner_document().unwrap();
    let b = element(&doc, "button");
    b.set_text_content(Some(label));
    css(
        &b,
        &[
            ("min-height", "48px"),
            ("padding", "10px 16px"),
            ("margin", "6px"),
            ("font", "inherit"),
            ("cursor", "pointer"),
            ("color", "#fff"),
            ("background", "#233354"),
            ("border", "1px solid #52607e"),
            ("border-radius", "16px"),
        ],
    );
    let _ = parent.append_child(&b);
    b
}
fn line(parent: &HtmlElement, words: &str) -> HtmlElement {
    let p = element(&parent.owner_document().unwrap(), "p");
    p.set_text_content(Some(words));
    let _ = parent.append_child(&p);
    p
}
async fn fetch(path: &str, body: Option<String>) -> Result<String, String> {
    let init = RequestInit::new();
    if let Some(body) = body {
        init.set_method("POST");
        init.set_body(&body.into());
    }
    let request =
        Request::new_with_str_and_init(path, &init).map_err(|_| "Builds could not be loaded.")?;
    request
        .headers()
        .set("Content-Type", "application/json")
        .map_err(|_| "Builds could not be loaded.")?;
    let response: Response =
        JsFuture::from(web_sys::window().unwrap().fetch_with_request(&request))
            .await
            .map_err(|_| "Check your connection and try again.")?
            .dyn_into()
            .map_err(|_| "Builds could not be loaded.")?;
    let text = JsFuture::from(response.text().map_err(|_| "Builds could not be loaded.")?)
        .await
        .map_err(|_| "Builds could not be loaded.")?
        .as_string()
        .ok_or("Builds could not be loaded.")?;
    if !response.ok() {
        return Err(if text.len() < 200 {
            text
        } else {
            "Builds could not be loaded. Try again.".into()
        });
    }
    Ok(text)
}
fn sign(job: usize, run: &Receipt, gallery: bool) -> Result<Shared, String> {
    let window = web_sys::window().unwrap();
    let crypto = window
        .crypto()
        .map_err(|_| "This browser could not make a share file.")?;
    let storage = window
        .local_storage()
        .ok()
        .flatten()
        .ok_or("Allow this site to save data before sharing.")?;
    let mut key = [0; 32];
    if let Some(saved) = storage.get_item(KEY).ok().flatten() {
        if saved.len() != 64 {
            return Err("This browser's sharing key could not be read.".into());
        }
        for (i, b) in key.iter_mut().enumerate() {
            *b = u8::from_str_radix(
                saved
                    .get(i * 2..i * 2 + 2)
                    .ok_or("The sharing key could not be read.")?,
                16,
            )
            .map_err(|_| "The sharing key could not be read.")?;
        }
    } else {
        crypto
            .get_random_values_with_u8_array(&mut key)
            .map_err(|_| "This browser could not make a share file.")?;
        storage
            .set_item(KEY, &receipt::hex(&key))
            .map_err(|_| "Allow this site to save data before sharing.")?;
    }
    let mut nonce = [0; 16];
    crypto
        .get_random_values_with_u8_array(&mut nonce)
        .map_err(|_| "This browser could not make a share file.")?;
    Shared::sign(job, run, &key, &nonce, gallery)
}
fn download(shared: &Shared) -> Result<(), String> {
    let parts = js_sys::Array::new();
    parts.push(&shared.encode().into());
    let blob = web_sys::Blob::new_with_str_sequence(&parts)
        .map_err(|_| "The build could not be downloaded.")?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "The build could not be downloaded.")?;
    let doc = web_sys::window().unwrap().document().unwrap();
    let a: web_sys::HtmlAnchorElement = element(&doc, "a").unchecked_into();
    a.set_href(&url);
    a.set_download(&format!("snap-{}.json", shared.code()));
    let _ = doc.body().unwrap().append_child(&a);
    a.click();
    a.remove();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}
impl Sharing {
    pub fn new(parent: &web_sys::Element) -> Self {
        let doc = parent.owner_document().unwrap();
        let root = element(&doc, "div");
        root.set_id("snap-sharing");
        let _ = root.set_attribute("role", "dialog");
        let _ = root.set_attribute("aria-modal", "true");
        let _ = root.set_attribute("aria-label", "Shared builds");
        css(
            &root,
            &[
                ("display", "none"),
                ("position", "fixed"),
                ("inset", "0"),
                ("z-index", "20"),
                ("background", "#070b1e"),
                ("overflow", "auto"),
                ("padding", "16px"),
                ("color", "#fff"),
                ("font", "18px system-ui"),
            ],
        );
        let face = element(&doc, "div");
        css(
            &face,
            &[
                ("max-width", "650px"),
                ("margin", "auto"),
                ("padding-bottom", "40px"),
            ],
        );
        let _ = root.append_child(&face);
        let _ = parent.append_child(&root);
        Self {
            root,
            face,
            imports: Rc::default(),
        }
    }
    pub fn active(&self) -> bool {
        self.root
            .style()
            .get_property_value("display")
            .unwrap_or_default()
            != "none"
    }
    pub fn take(&self) -> Vec<(usize, Vec<Input>)> {
        self.imports.borrow_mut().drain(..).collect()
    }
    fn clear(&self, title: &str) {
        self.face.set_text_content(None);
        let heading = element(&self.face.owner_document().unwrap(), "h2");
        heading.set_text_content(Some(title));
        let _ = self.face.append_child(&heading);
        let this = self.clone();
        click(&button(&self.face, "Close"), move || {
            css(&this.root, &[("display", "none")]);
            if let Some(hud) = this
                .face
                .owner_document()
                .unwrap()
                .query_selector(".snap-hud")
                .ok()
                .flatten()
            {
                let _ = hud.remove_attribute("inert");
            }
        });
        css(&self.root, &[("display", "block")]);
        if let Some(hud) = self
            .face
            .owner_document()
            .unwrap()
            .query_selector(".snap-hud")
            .ok()
            .flatten()
        {
            let _ = hud.set_attribute("inert", "");
        }
        if let Some(b) = self
            .face
            .query_selector("button")
            .ok()
            .flatten()
            .and_then(|b| b.dyn_into::<HtmlElement>().ok())
        {
            let _ = b.focus();
        }
    }
    pub fn menu(&self, job: usize, run: Option<Receipt>) {
        self.clear("Shared builds");
        if let Some(run) = run {
            let label = element(&self.face.owner_document().unwrap(), "label");
            let check: HtmlInputElement =
                element(&self.face.owner_document().unwrap(), "input").unchecked_into();
            check.set_type("checkbox");
            check.set_id("snap-gallery-consent");
            let _ = label.append_child(&check);
            let _ = label.append_child(
                &self
                    .face
                    .owner_document()
                    .unwrap()
                    .create_text_node(" Show in the public gallery"),
            );
            let _ = self.face.append_child(&label);
            let status = line(&self.face, "");
            let _ = status.set_attribute("role", "status");
            for online in [true, false] {
                let (this, run, check, status) =
                    (self.clone(), run.clone(), check.clone(), status.clone());
                click(
                    &button(
                        &self.face,
                        if online {
                            "Get a share code"
                        } else {
                            "Download build"
                        },
                    ),
                    move || {
                        let shared = match sign(job, &run, check.checked()) {
                            Ok(s) => s,
                            Err(e) => {
                                status.set_text_content(Some(&e));
                                return;
                            }
                        };
                        if !online {
                            if let Err(e) = download(&shared) {
                                status.set_text_content(Some(&e));
                            }
                            return;
                        }
                        let (this, status) = (this.clone(), status.clone());
                        status.set_text_content(Some("Saving your build…"));
                        spawn_local(async move {
                            match fetch(API, Some(shared.encode())).await {
                                Ok(text) => {
                                    let code = serde_json::from_str::<serde_json::Value>(&text)
                                        .ok()
                                        .and_then(|v| v["code"].as_str().map(str::to_owned));
                                    if let Some(code) = code.and_then(|s| snap_share::code(&s)) {
                                        status.set_text_content(Some(&format!(
                                            "Your code: {}-{}-{}",
                                            &code[..4],
                                            &code[4..8],
                                            &code[8..]
                                        )));
                                        let a = element(&this.face.owner_document().unwrap(), "a");
                                        let _ = a.set_attribute(
                                            "href",
                                            &format!("/games/snap-station?share={code}"),
                                        );
                                        a.set_text_content(Some("Open your share link"));
                                        css(&a, &[("color", "#7cf2c8")]);
                                        let _ = this.face.append_child(&a);
                                    } else {
                                        status
                                            .set_text_content(Some("The code could not be read."));
                                    }
                                }
                                Err(e) => status.set_text_content(Some(&e)),
                            }
                        });
                    },
                );
            }
        } else {
            line(&self.face, "Finish a run to share your build.");
        }
        let input: HtmlInputElement =
            element(&self.face.owner_document().unwrap(), "input").unchecked_into();
        input.set_placeholder("Code from a friend");
        let _ = input.set_attribute("aria-label", "Build code");
        input.set_max_length(32);
        input.set_id("snap-share-code");
        let _ = self.face.append_child(&input);
        let this = self.clone();
        click(&button(&self.face, "Open code"), move || {
            this.open_code(&input.value())
        });
        let file: HtmlInputElement =
            element(&self.face.owner_document().unwrap(), "input").unchecked_into();
        file.set_type("file");
        file.set_accept(".json,application/json");
        let _ = file.set_attribute("aria-label", "Open a build file");
        let _ = self.face.append_child(&file);
        let (this, field) = (self.clone(), file.clone());
        let callback = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            let Some(file) = field.files().and_then(|f| f.get(0)) else {
                return;
            };
            if file.size() > snap_share::MAX_BYTES as f64 {
                line(&this.face, "This build file is too large.");
                return;
            }
            let this = this.clone();
            spawn_local(async move {
                match JsFuture::from(file.text())
                    .await
                    .ok()
                    .and_then(|t| t.as_string())
                    .ok_or("The build file could not be read.".to_owned())
                    .and_then(|s| Shared::decode(&s))
                {
                    Ok(s) => this.preview(s),
                    Err(e) => {
                        line(&this.face, &e);
                    }
                }
            });
        });
        let function = callback.into_js_value();
        let _ = file.add_event_listener_with_callback("change", function.unchecked_ref());
        let this = self.clone();
        click(&button(&self.face, "Visit gallery"), move || this.gallery());
    }
    pub fn open_code(&self, code: &str) {
        self.clear("Opening build");
        let Some(code) = snap_share::code(code) else {
            line(&self.face, "Enter the 12-character code from your friend.");
            return;
        };
        let this = self.clone();
        spawn_local(async move {
            match fetch(&format!("{API}/{code}"), None)
                .await
                .and_then(|s| Shared::decode(&s))
            {
                Ok(shared) => this.preview(shared),
                Err(e) => {
                    line(&this.face, &e);
                }
            }
        });
    }
    fn preview(&self, shared: Shared) {
        self.clear("Shared station");
        match shared.check() {
            Ok(checked) => self.entry(shared, checked),
            Err(e) => {
                line(&self.face, &e);
            }
        }
    }
    fn entry(&self, shared: Shared, checked: snap_share::Checked) {
        let row = element(&self.face.owner_document().unwrap(), "section");
        css(
            &row,
            &[("border-top", "1px solid #52607e"), ("padding", "12px 0")],
        );
        let _ = row.set_attribute("data-share-code", &shared.code());
        line(
            &row,
            &format!(
                "Job {} · {} · {} stars",
                shared.job, checked.game.level.name, checked.game.stars
            ),
        );
        // The thumbnail uses only numbers from the game that just replayed.
        let mut points = Vec::new();
        for p in &checked.game.build.pieces {
            points.extend(p.at.iter().copied());
        }
        let (mut lo, mut hi) = (snap_rules::V::new(-1., -1.), snap_rules::V::new(1., 1.));
        for p in points {
            lo.x = lo.x.min(p.x);
            lo.y = lo.y.min(p.y);
            hi.x = hi.x.max(p.x);
            hi.y = hi.y.max(p.y);
        }
        let mut svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' role='img' aria-label='Shared station' viewBox='{} {} {} {}' width='260' height='140'>",
            lo.x - 0.5,
            -hi.y - 0.5,
            hi.x - lo.x + 1.,
            hi.y - lo.y + 1.
        );
        for p in &checked.game.build.pieces {
            svg.push_str("<polygon fill='#3ba7f5' stroke='#fff' stroke-width='.04' points='");
            for at in &p.at {
                svg.push_str(&format!("{},{} ", at.x, -at.y));
            }
            svg.push_str("'/>");
        }
        svg.push_str("</svg>");
        let preview = element(&self.face.owner_document().unwrap(), "div");
        preview.set_inner_html(&svg);
        let _ = row.append_child(&preview);
        let this = self.clone();
        click(&button(&row, "Build from this"), move || {
            this.imports
                .borrow_mut()
                .push((shared.job, checked.construction.clone()));
            css(&this.root, &[("display", "none")]);
            if let Some(hud) = this
                .face
                .owner_document()
                .unwrap()
                .query_selector(".snap-hud")
                .ok()
                .flatten()
            {
                let _ = hud.remove_attribute("inert");
            }
        });
        let _ = self.face.append_child(&row);
    }
    fn gallery(&self) {
        self.clear("Station gallery");
        let status = line(&self.face, "Playing the shared runs…");
        let this = self.clone();
        spawn_local(async move {
            match fetch(API, None).await.and_then(|s| {
                serde_json::from_str::<Vec<Shared>>(&s)
                    .map_err(|_| "The gallery could not be read.".into())
            }) {
                Ok(builds) => {
                    status.set_text_content(Some(""));
                    let mut count = 0;
                    for shared in builds.into_iter().take(24) {
                        if shared.gallery {
                            if let Ok(checked) = shared.check() {
                                this.entry(shared, checked);
                                count += 1;
                            }
                        }
                    }
                    if count == 0 {
                        status.set_text_content(Some("No stations have been shared here yet."));
                    }
                }
                Err(e) => status.set_text_content(Some(&e)),
            }
        });
    }
}
