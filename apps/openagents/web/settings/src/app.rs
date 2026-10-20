//! Settings' pages (#12288, #12289, #12290): the desktop's forms from
//! `openagents_chat_view::settings_forms`, mounted with the shared GPUI
//! adapter, in a window over the chat. Device choices are saved to this
//! browser's storage and the chat is told (`oa-settings-changed`) so it
//! applies them; a computer's settings go to that computer over host RPC;
//! the account's go to the account API with the sign-in cookie.

use gpui::{
    ClickEvent, ClipboardItem, Context, FocusHandle, IntoElement, KeyDownEvent, ParentElement,
    Render, Styled, Task, Window, div, prelude::*, px,
};
use openagents_chat_view::settings_forms::{
    self as forms, Archived, FilesPrefs, Intent, Load, Named, Page, Paints, Prefs, Remote, Titles,
    Worktrees, account, appearance, shortcuts,
};
use openagents_chat_view::{rust_native_gpui, theme::Theme};
use openagents_web_chat_view::theme;
use openagents_web_client::web::Client;
use openagents_web_client::{Refusal, computer, store};
use serde::Deserialize;
use serde_json::{Value, json};
use wasm_bindgen::{JsCast, closure::Closure};
use wasm_bindgen_futures::JsFuture;

/// What the chat knows that a page shows: the account's computers and
/// archived chats, from its registry, and whether it's signed in.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Shared {
    pub computers: Vec<SharedComputer>,
    pub archived: Vec<SharedChat>,
    pub signed_in: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct SharedComputer {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct SharedChat {
    pub id: String,
    pub title: String,
    pub detail: String,
}

/// The chat's [`Shared`] as the pages read it.
pub struct Known {
    pub computers: Vec<Named>,
    pub archived: Vec<Archived>,
    pub signed_in: bool,
}

impl From<Shared> for Known {
    fn from(shared: Shared) -> Self {
        Self {
            computers: shared
                .computers
                .into_iter()
                .map(|c| Named::new(c.id, c.name))
                .collect(),
            archived: shared
                .archived
                .into_iter()
                .map(|c| Archived {
                    id: c.id,
                    title: c.title,
                    detail: c.detail,
                })
                .collect(),
            signed_in: shared.signed_in,
        }
    }
}

pub struct SettingsApp {
    client: Client,
    page: Page,
    prefs: Prefs,
    remote: Remote,
    context: Known,
    revision: u64,
    notice: Option<String>,
    system_dark: bool,
    focus: FocusHandle,
    focus_wanted: bool,
    computer_task: Option<Task<()>>,
    account_task: Option<Task<()>>,
}

impl SettingsApp {
    pub fn new(page: Page, shared: Shared, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            client: Client::from_page(),
            page,
            prefs: Prefs::default(),
            remote: Remote {
                mac: is_mac(),
                ..Remote::default()
            },
            context: shared.into(),
            revision: 1,
            notice: None,
            system_dark: prefers_dark(),
            focus: cx.focus_handle(),
            focus_wanted: false,
            computer_task: None,
            account_task: None,
        };
        openagents_chat_view::motion::init(
            this.prefs.appearance.reduce_motion,
            this.prefs.appearance.pause_in_background,
            cx,
        );
        openagents_chat_view::motion::set_system(prefers_reduced_motion(), cx);
        this.reload(cx);
        this.show(page, false, cx);
        this
    }

    /// Read the stored choices again: the chat or the files panel may have
    /// changed them.
    fn reload(&mut self, cx: &mut Context<Self>) {
        self.prefs = Prefs::from_json(store::get(Prefs::STORAGE_KEY).as_deref());
        self.prefs.files = FilesPrefs::from_json(store::get(FilesPrefs::STORAGE_KEY).as_deref());
        // The composer's own switch keeps the send key too.
        if let Some(on) = store::get("openagents.composer.send-with-modifier") {
            self.prefs.send_with_modifier = on == "1";
        }
        self.apply_appearance(cx);
    }

    fn save(&mut self) {
        // The chat reads the imported themes compiled, in postcard (which it
        // links already), so it needs neither the importer nor a JSON reader
        // for them.
        use base64::Engine as _;
        let families = self.prefs.appearance.families();
        let packed = postcard::to_allocvec(&families).unwrap_or_default();
        store::set(
            appearance::Appearance::COMPILED_KEY,
            &base64::engine::general_purpose::STANDARD.encode(packed),
        );
        store::set(Prefs::STORAGE_KEY, &self.prefs.to_json());
        store::set(FilesPrefs::STORAGE_KEY, &self.prefs.files.to_json());
        store::signal("oa-settings-changed", "");
    }

    /// Draw this window in the chosen theme too.
    fn apply_appearance(&mut self, cx: &mut Context<Self>) {
        let look = &self.prefs.appearance;
        theme::set(look.theme(self.system_dark), cx);
        openagents_chat_view::motion::set_preference(look.reduce_motion, cx);
        cx.refresh_windows();
    }

    /// Show `page` with what the chat knows now.
    pub fn open(&mut self, page: Page, shared: Shared, cx: &mut Context<Self>) {
        self.context = shared.into();
        self.reload(cx);
        self.show(page, false, cx);
    }

    fn show(&mut self, page: Page, push: bool, cx: &mut Context<Self>) {
        if push {
            store::signal("oa-settings-page", page.slug());
        }
        self.page = page;
        self.remote.recording = None;
        self.remote.shortcut_notice = None;
        self.remote.archived = self.context.archived.clone();
        match page {
            Page::General => self.load_computer_settings(cx),
            Page::Account => self.load_account(cx),
            Page::Notifications => self.remote.notifications_allowed = notifications_allowed(),
            _ => {}
        }
        self.revision += 1;
        cx.notify();
    }

    /// Thread naming and the worktree folder of the chosen computer.
    fn load_computer_settings(&mut self, cx: &mut Context<Self>) {
        let computers = self.context.computers.clone();
        let remote = &mut self.remote;
        if remote
            .computer
            .as_ref()
            .is_none_or(|id| !computers.iter().any(|c| &c.id == id))
        {
            remote.computer = computers.first().map(|c| c.id.clone());
        }
        remote.computers = computers;
        let Some(device) = remote.computer.clone() else {
            return;
        };
        remote.titles = Load::Loading;
        remote.worktrees = Load::Loading;
        let name = self.computer_name(&device);
        self.computer_task = Some(self.computer_calls(
            device.clone(),
            name,
            vec![
                (computer::LIST_HARNESSES, json!({})),
                ("GetTitleSettings", json!({})),
                ("GetWorktreeSettings", json!({})),
            ],
            move |this, mut answers, cx| {
                let worktrees = answers.pop().unwrap_or(Err(String::new()));
                let titles = answers.pop().unwrap_or(Err(String::new()));
                let harnesses = answers.pop().unwrap_or(Err(String::new()));
                this.remote.worktrees = match worktrees {
                    Ok(value) => Load::Ready(worktrees_of(&value)),
                    Err(error) => Load::Failed(error),
                };
                match (titles, harnesses) {
                    (Ok(titles), Ok(harnesses)) => {
                        let list: Vec<openagents_web_client::workspace::Harness> =
                            serde_json::from_value(harnesses).unwrap_or_default();
                        let harnesses = list
                            .into_iter()
                            .filter(openagents_web_client::workspace::Harness::usable)
                            .map(|h| Named::new(id_text(&h.id), h.name))
                            .collect();
                        this.remote.titles = Load::Ready(Titles {
                            harness: titles["harness"].as_str().map(str::to_owned),
                            model: titles["model"].as_str().map(str::to_owned),
                            harnesses,
                            models: Vec::new(),
                        });
                        this.load_title_models(cx);
                    }
                    (Err(error), _) | (_, Err(error)) => this.remote.titles = Load::Failed(error),
                }
            },
            cx,
        ));
    }

    fn load_title_models(&mut self, cx: &mut Context<Self>) {
        let Some(device) = self.remote.computer.clone() else {
            return;
        };
        let Some(harness) = self.remote.titles.ready().and_then(|t| t.harness.clone()) else {
            return;
        };
        let name = self.computer_name(&device);
        self.computer_task = Some(self.computer_calls(
            device,
            name,
            vec![(computer::LIST_MODELS, json!({ "harness": harness }))],
            move |this, mut answers, _| {
                let Some(Ok(value)) = answers.pop() else {
                    return;
                };
                if let Load::Ready(titles) = &mut this.remote.titles
                    && titles.harness.as_deref() == Some(harness.as_str())
                {
                    titles.models = value
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|m| {
                            Some(Named::new(
                                m["id"].as_str()?,
                                m["label"].as_str().or(m["id"].as_str())?,
                            ))
                        })
                        .collect();
                }
            },
            cx,
        ));
    }

    fn computer_name(&self, device: &str) -> String {
        self.remote
            .computers
            .iter()
            .find(|c| c.id == device)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "your computer".into())
    }

    /// Call `calls` on `device` in turn and hand every answer to `then`.
    fn computer_calls(
        &mut self,
        device: String,
        name: String,
        calls: Vec<(&'static str, Value)>,
        then: impl FnOnce(&mut Self, Vec<Result<Value, String>>, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        self.computer_calls_boxed(device, name, calls, Box::new(then), cx)
    }

    /// One copy of the call loop for every caller.
    fn computer_calls_boxed(
        &mut self,
        device: String,
        name: String,
        calls: Vec<(&'static str, Value)>,
        then: Box<dyn FnOnce(&mut Self, Vec<Result<Value, String>>, &mut Context<Self>)>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let asked = {
                let this = this.clone();
                let mut cx = cx.clone();
                move || {
                    let _ = this.update(&mut cx, |this, cx| {
                        this.notice = Some(allow_line(&name));
                        cx.notify();
                    });
                }
            };
            let mut answers = Vec::new();
            match computer::connect(&client, &device, asked).await {
                Ok(open) => {
                    for (method, params) in calls {
                        answers.push(open.call(method, params).await.map_err(|e| plain(&e)));
                    }
                }
                Err(error) => answers = vec![Err(plain(&error)); calls.len()],
            }
            this.update(cx, |this, cx| {
                if this.remote.computer.as_deref() != Some(device.as_str()) {
                    return;
                }
                then(this, answers, cx);
                this.revision += 1;
                cx.notify();
            })
            .ok();
        })
    }

    fn load_account(&mut self, cx: &mut Context<Self>) {
        self.remote.account.signed_in = self.context.signed_in;
        if !self.context.signed_in {
            return;
        }
        if !matches!(self.remote.account.keys, Load::Ready(_)) {
            self.remote.account.keys = Load::Loading;
        }
        let client = self.client.clone();
        self.account_task = Some(cx.spawn(async move |this, cx| {
            let account: Result<Value, _> = client.get("/v1/account").await;
            let keys: Result<Value, _> = client.get("/v1/device/keys").await;
            let name: Result<Value, _> = client.get("/v1/device/name").await;
            this.update(cx, |this, cx| {
                let state = &mut this.remote.account;
                if let Ok(account) = account {
                    let account = &account["account"];
                    state.label = account["email"]
                        .as_str()
                        .or(account["label"].as_str())
                        .map(str::to_owned);
                }
                state.name = name
                    .ok()
                    .and_then(|n| n["name"].as_str().map(str::to_owned))
                    .filter(|n| !n.is_empty());
                state.keys = match keys {
                    Ok(keys) => Load::Ready(
                        serde_json::from_value(keys["keys"].clone()).unwrap_or_default(),
                    ),
                    Err(error) => Load::Failed(plain(&error)),
                };
                this.revision += 1;
                cx.notify();
            })
            .ok();
        }));
    }

    fn settings_intent(&mut self, intent: Intent, cx: &mut Context<Self>) {
        self.revision += 1;
        if let Intent::Shortcut(shortcuts::Change::Record(shortcut)) = &intent {
            self.remote.recording = Some(*shortcut);
            self.remote.shortcut_notice = None;
            // The next frame puts the keyboard on the page to hear the chord.
            self.focus_wanted = true;
            cx.notify();
            return;
        }
        if let Intent::Shortcut(shortcuts::Change::StopRecording) = &intent {
            self.remote.recording = None;
            cx.notify();
            return;
        }
        let before = self.prefs.clone();
        if self.prefs.apply(&intent) {
            self.save();
            if self.prefs.notifications_enabled && !before.notifications_enabled {
                ask_permission();
            }
            if self.prefs.appearance != before.appearance {
                self.apply_appearance(cx);
            }
            self.remote.notifications_allowed = notifications_allowed();
            cx.notify();
            return;
        }
        match intent {
            Intent::Open(page) => self.show(page, true, cx),
            Intent::Computer(id) => {
                self.remote.computer = Some(id);
                self.load_computer_settings(cx);
            }
            Intent::TitleHarness(harness) => {
                self.save_titles(json!({ "harness": harness, "model": null }), cx)
            }
            Intent::TitleModel(model) => {
                let harness = self.remote.titles.ready().and_then(|t| t.harness.clone());
                self.save_titles(json!({ "harness": harness, "model": model }), cx)
            }
            Intent::WorktreeBrowse(None) => {
                if let Load::Ready(w) = &mut self.remote.worktrees {
                    w.browsing = None;
                }
            }
            Intent::WorktreeBrowse(Some(path)) => self.browse_folder(path, cx),
            Intent::WorktreeUse(path) => self.save_worktrees(
                json!({ "useCustomDirectory": true, "customDirectory": path }),
                cx,
            ),
            Intent::WorktreeDefault => {
                let custom = self.remote.worktrees.ready().map(|w| w.effective.clone());
                self.save_worktrees(
                    json!({ "useCustomDirectory": false, "customDirectory": custom }),
                    cx,
                )
            }
            Intent::Unarchive(id) => {
                self.context.archived.retain(|chat| chat.id != id);
                store::signal("oa-settings-chat", &json!({ "unarchive": id }).to_string());
            }
            Intent::ConfirmDeleteAll(ask) => self.remote.confirm_delete_all = ask,
            Intent::DeleteAllArchived => {
                self.remote.confirm_delete_all = false;
                for chat in std::mem::take(&mut self.context.archived) {
                    store::signal(
                        "oa-settings-chat",
                        &json!({ "delete": chat.id }).to_string(),
                    );
                }
            }
            Intent::ArchivedPage(page) => self.remote.archived_page = page,
            Intent::Appearance(appearance::Change::ChooseBackground) => self.choose_background(cx),
            Intent::Appearance(appearance::Change::ImportTheme) => self.import_theme(cx),
            Intent::Account(action) => self.account_action(action, cx),
            _ => {}
        }
        cx.notify();
    }

    fn save_titles(&mut self, settings: Value, cx: &mut Context<Self>) {
        let Some(device) = self.remote.computer.clone() else {
            return;
        };
        let name = self.computer_name(&device);
        self.computer_task = Some(self.computer_calls(
            device,
            name,
            vec![("SetTitleSettings", settings)],
            |this, mut answers, cx| match answers.pop() {
                Some(Ok(saved)) => {
                    if let Load::Ready(titles) = &mut this.remote.titles {
                        let harness = saved["harness"].as_str().map(str::to_owned);
                        if titles.harness != harness {
                            titles.models.clear();
                        }
                        titles.harness = harness;
                        titles.model = saved["model"].as_str().map(str::to_owned);
                    }
                    this.load_title_models(cx);
                }
                Some(Err(error)) => this.notice = Some(error),
                None => {}
            },
            cx,
        ));
    }

    fn save_worktrees(&mut self, settings: Value, cx: &mut Context<Self>) {
        let Some(device) = self.remote.computer.clone() else {
            return;
        };
        let name = self.computer_name(&device);
        self.computer_task = Some(self.computer_calls(
            device,
            name,
            vec![("SetWorktreeSettings", settings)],
            |this, mut answers, _| match answers.pop() {
                Some(Ok(saved)) => this.remote.worktrees = Load::Ready(worktrees_of(&saved)),
                Some(Err(error)) => this.notice = Some(error),
                None => {}
            },
            cx,
        ));
    }

    fn browse_folder(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(device) = self.remote.computer.clone() else {
            return;
        };
        let name = self.computer_name(&device);
        self.computer_task = Some(self.computer_calls(
            device,
            name,
            vec![("ListFolders", json!({ "path": path }))],
            |this, mut answers, _| match answers.pop() {
                Some(Ok(listing)) => {
                    if let Load::Ready(w) = &mut this.remote.worktrees {
                        let at = listing["path"].as_str().unwrap_or_default().to_owned();
                        let children = listing["entries"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter(|e| e["isDir"].as_bool().unwrap_or(true))
                            .filter_map(|e| e["name"].as_str())
                            .filter(|n| !n.starts_with('.'))
                            .map(|n| Named::new(format!("{}/{n}", at.trim_end_matches('/')), n))
                            .collect();
                        let truncated = listing["truncated"].as_bool().unwrap_or(false);
                        w.browsing = Some((at, children, truncated));
                    }
                }
                Some(Err(error)) => this.notice = Some(error),
                None => {}
            },
            cx,
        ));
    }

    fn choose_background(&mut self, cx: &mut Context<Self>) {
        let picked = pick_file("image/png,image/jpeg,image/webp,image/gif");
        cx.spawn(async move |this, cx| {
            let Some((name, bytes)) = picked.await else {
                return;
            };
            if bytes.len() > 20 * 1024 * 1024 {
                this.update(cx, |this, cx| {
                    this.notice = Some("That image is over 20 MB. Choose a smaller one.".into());
                    cx.notify();
                })
                .ok();
                return;
            }
            let saved = store::write(store::BACKGROUND, &bytes).await;
            this.update(cx, |this, cx| {
                if saved.is_err() {
                    this.notice = Some("This browser couldn't keep that image.".into());
                    cx.notify();
                    return;
                }
                let change = Intent::Appearance(appearance::Change::SetBackground(name));
                // Saved even when the name is the same: the picture changed.
                this.prefs.apply(&change);
                this.save();
                this.revision += 1;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn import_theme(&mut self, cx: &mut Context<Self>) {
        let picked = pick_file(".json,.jsonc,application/json");
        cx.spawn(async move |this, cx| {
            let Some((name, bytes)) = picked.await else {
                return;
            };
            this.update(cx, |this, cx| {
                let stem = name.trim_end_matches(".jsonc").trim_end_matches(".json");
                let answer = match String::from_utf8(bytes) {
                    Err(_) => Err("That file isn't text.".to_string()),
                    Ok(source) if source.len() > appearance::LibraryTheme::MAX_SOURCE => {
                        Err("That theme file is too large.".to_string())
                    }
                    Ok(source) => {
                        let theme = appearance::LibraryTheme {
                            id: format!("web-{}", slug(stem)),
                            name: stem.to_owned(),
                            file: name.clone(),
                            source,
                        };
                        match theme.compile() {
                            Some(family) => Ok((theme, family)),
                            None => Err(
                                "We couldn't read that theme. Choose a VS Code theme's .json file."
                                    .to_string(),
                            ),
                        }
                    }
                };
                match answer {
                    Ok((mut theme, family)) => {
                        if let Some(variant) = family.variants.first() {
                            theme.name = variant.name.clone();
                        }
                        let scheme = family.variants.first().map(|v| v.appearance);
                        let id = theme.id.clone();
                        let theme_name = theme.name.clone();
                        let mut changed = this
                            .prefs
                            .apply(&Intent::Appearance(appearance::Change::Imported(theme)));
                        if let Some(scheme) = scheme {
                            changed |= this
                                .prefs
                                .apply(&Intent::Appearance(appearance::Change::Theme(scheme, id)));
                        }
                        if changed {
                            this.save();
                        }
                        this.remote.theme_notice = Some(format!("Added {theme_name}."));
                        this.apply_appearance(cx);
                    }
                    Err(line) => this.remote.theme_notice = Some(line),
                }
                this.revision += 1;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn account_action(&mut self, action: account::Action, cx: &mut Context<Self>) {
        use account::{Action, Deleting};
        let state = &mut self.remote.account;
        match action {
            Action::Copy(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.notice = None;
            }
            Action::ConfirmRemove(key) => state.confirm_remove = key,
            Action::Open(path) => go(&path),
            Action::SignOut => sign_out(self.client.csrf()),
            Action::SignIn => sign_in(),
            Action::AskDelete(true) => state.deleting = Some(Deleting::Ask),
            Action::AskDelete(false) => state.deleting = None,
            Action::RemoveKey(key) => {
                state.confirm_remove = None;
                state.removing = Some(key.clone());
                state.notice = None;
                let client = self.client.clone();
                cx.spawn(async move |this, cx| {
                    let path = format!("/v1/device/keys/{key}");
                    let removed: Result<Value, _> = client.send("DELETE", &path, None::<&()>).await;
                    this.update(cx, |this, cx| {
                        let state = &mut this.remote.account;
                        state.removing = None;
                        match removed {
                            Ok(answer) => {
                                if let Ok(keys) = serde_json::from_value(answer["keys"].clone()) {
                                    state.keys = Load::Ready(keys);
                                } else if let Load::Ready(keys) = &mut state.keys {
                                    keys.retain(|k| k.pubkey != key);
                                }
                            }
                            Err(error) => state.notice = Some(plain(&error)),
                        }
                        this.revision += 1;
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            Action::Delete(forfeit) => {
                state.deleting = Some(Deleting::Working);
                let client = self.client.clone();
                cx.spawn(async move |this, cx| {
                    let body = json!({
                        "confirm": account::DELETE_CONFIRM,
                        "forfeit_balance": forfeit,
                    });
                    let answer: Result<Value, _> =
                        client.send("DELETE", "/v1/account", Some(&body)).await;
                    this.update(cx, |this, cx| {
                        let step = match answer {
                            Ok(answer) if answer["account"]["state"] == "deleted" => {
                                go("/");
                                None
                            }
                            Ok(_) => Some(Deleting::Notice(
                                "We couldn't delete your account. Try again.".into(),
                            )),
                            Err(error) => Some(match error.downcast_ref::<Refusal>() {
                                Some(refusal) => account::refused(&refusal.code, &refusal.message),
                                None => account::refused("", ""),
                            }),
                        };
                        this.remote.account.deleting = step;
                        this.revision += 1;
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
        }
        self.revision += 1;
    }

    /// A key pressed while a shortcut records: the new chord, or Escape.
    fn record_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(shortcut) = self.remote.recording else {
            return;
        };
        let keystroke = &event.keystroke;
        if keystroke.key == "escape" && !keystroke.modifiers.modified() {
            self.remote.recording = None;
            cx.notify();
            return;
        }
        let m = &keystroke.modifiers;
        let mac = self.remote.mac;
        let Some(combo) = shortcuts::combo_from_keystroke_on(
            mac,
            m.control,
            m.alt,
            m.shift,
            m.platform,
            &keystroke.key,
        ) else {
            return;
        };
        cx.stop_propagation();
        self.remote.recording = None;
        match self.prefs.keymap.refusal(shortcut, &combo, mac) {
            Some(refusal) => self.remote.shortcut_notice = Some(refusal),
            None => {
                self.remote.shortcut_notice = None;
                if self
                    .prefs
                    .apply(&Intent::Shortcut(shortcuts::Change::Set(shortcut, combo)))
                {
                    self.save();
                }
            }
        }
        self.revision += 1;
        cx.notify();
    }
}

impl Render for SettingsApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_wanted) {
            window.focus(&self.focus, cx);
        }
        let wide = window.viewport_size().width > px(820.);
        let page = self.page;
        let paints = paints(&theme::desktop());
        let root = forms::page(page, &self.prefs, &self.remote, &paints);
        let body = rust_native_gpui::mount_root(
            &root,
            "settings",
            self.revision,
            move |this: &mut Self, activation, cx| {
                let root = forms::page(this.page, &this.prefs, &this.remote, &paints);
                if let Some(intent) = find_intent(&root, &activation.node) {
                    this.settings_intent(intent, cx);
                }
            },
            cx,
        );
        let nav = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .when(wide, |nav| {
                nav.w(px(220.)).flex_none().pt(px(16.)).px(px(10.))
            })
            .when(!wide, |nav| {
                nav.flex_row().flex_wrap().px(px(12.)).pt(px(10.))
            })
            .children(Page::ALL.into_iter().map(|p| {
                let chosen = p == page;
                div()
                    .id(gpui::SharedString::from(format!(
                        "settings-nav-{}",
                        p.slug()
                    )))
                    .when(wide && p.starts_group(), |row| row.mt(px(14.)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .px(px(10.))
                    .h(px(32.))
                    .rounded(px(8.))
                    .text_size(px(13.5))
                    .cursor_pointer()
                    .text_color(if chosen {
                        theme::text()
                    } else {
                        theme::text_muted()
                    })
                    .when(chosen, |row| row.bg(theme::element_active()))
                    .hover(|s| s.bg(theme::element_hover()).text_color(theme::text()))
                    .on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.show(p, true, cx)),
                    )
                    .child(p.label())
            }));
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .h(px(52.))
            .px(px(16.))
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .id("settings-back")
                    .px(px(8.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .text_size(px(13.5))
                    .text_color(theme::text_muted())
                    .hover(|s| s.bg(theme::element_hover()).text_color(theme::text()))
                    .on_click(|_: &ClickEvent, _, _| store::signal("oa-settings-close", ""))
                    .child("\u{2190} Back to chat"),
            )
            .child(
                div()
                    .flex_1()
                    .text_size(px(14.5))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(format!("Settings \u{b7} {}", page.label())),
            );
        let notice = self.notice.clone().map(|line| {
            div()
                .id("settings-notice")
                .mx(px(16.))
                .mt(px(10.))
                .px(px(12.))
                .py(px(8.))
                .rounded(px(10.))
                .bg(theme::danger().opacity(0.08))
                .border_1()
                .border_color(theme::danger().opacity(0.25))
                .text_size(px(13.))
                .text_color(theme::danger())
                .cursor_pointer()
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.notice = None;
                    cx.notify();
                }))
                .child(line)
        });
        let mut root = div()
            .id("openagents-web-settings")
            .track_focus(&self.focus)
            .on_key_down(
                cx.listener(|this, event: &KeyDownEvent, _, cx| this.record_key(event, cx)),
            )
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::bg())
            .font_family(theme::SANS)
            .text_color(theme::text())
            .text_size(px(theme::BODY));
        root.text_style().font_fallbacks = Some(theme::fallbacks());
        root.child(header).children(notice).child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .when(wide, |d| d.flex_row())
                .when(!wide, |d| d.flex_col())
                .child(nav)
                .child(
                    div()
                        .id("settings-page")
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .overflow_y_scroll()
                        .child(div().w_full().max_w(px(760.)).mx_auto().child(body)),
                ),
        )
    }
}

/// What the person at the computer has to do the first time.
fn allow_line(computer: &str) -> String {
    format!(
        "Allow this browser on {computer}: in the OpenAgents app there, open Settings, then Devices."
    )
}

fn notifications_allowed() -> Option<bool> {
    match web_sys::Notification::permission() {
        web_sys::NotificationPermission::Granted => Some(true),
        web_sys::NotificationPermission::Denied => Some(false),
        _ => None,
    }
}

/// Ask the browser to let the page show notifications, if it hasn't asked.
fn ask_permission() {
    if web_sys::Notification::permission() == web_sys::NotificationPermission::Default {
        let _ = web_sys::Notification::request_permission();
    }
}

/// Leave the canvas for another page of the site.
fn go(url: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().assign(url);
    }
}

/// Sign in, then come back here.
fn sign_in() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let here = window.location().pathname().unwrap_or_default();
    let back = String::from(js_sys::encode_uri_component(&here));
    go(&format!("/login?return_to={back}"));
}

/// The intent of the control `key` names in `root`.
fn find_intent(root: &rust_native::Node<Intent>, key: &str) -> Option<Intent> {
    use rust_native::Element;
    if root.key == key {
        return match &root.element {
            Element::Button {
                intent,
                enabled: true,
                ..
            }
            | Element::Choice {
                intent,
                enabled: true,
                ..
            } => Some(intent.clone()),
            _ => None,
        };
    }
    match &root.element {
        Element::Stack { children, .. }
        | Element::List { children, .. }
        | Element::Choice { children, .. }
        | Element::Dialog { children, .. } => children.iter().find_map(|c| find_intent(c, key)),
        _ => None,
    }
}

/// The forms' colors from the theme the page draws with.
fn paints(theme: &Theme) -> Paints {
    let color = |c: gpui::Hsla| {
        let rgba = c.to_rgb();
        rust_native::style::Color {
            red: (rgba.r * 255.).round() as u8,
            green: (rgba.g * 255.).round() as u8,
            blue: (rgba.b * 255.).round() as u8,
            alpha: (rgba.a * 255.).round() as u8,
        }
    };
    Paints {
        text: color(theme.text),
        muted: color(theme.text_muted),
        faint: color(theme.text_faint),
        card: color(theme.surface_card),
        border: color(theme.border),
        danger: color(theme.danger),
    }
}

fn worktrees_of(value: &Value) -> Worktrees {
    Worktrees {
        custom: value["settings"]["useCustomDirectory"]
            .as_bool()
            .unwrap_or(false),
        effective: value["effectiveDirectory"]
            .as_str()
            .unwrap_or_default()
            .into(),
        default: value["defaultDirectory"]
            .as_str()
            .unwrap_or_default()
            .into(),
        environment: value["environmentOverride"].as_str().map(str::to_owned),
        browsing: None,
    }
}

fn id_text(id: &openagents_proto::HarnessId) -> String {
    serde_json::to_value(id)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// A computer's answer in the person's words.
fn plain(error: &anyhow::Error) -> String {
    let text = error.to_string();
    if text.contains("can't do that") || text.contains("Unknown method") {
        "Update OpenAgents on this computer to change this from the web.".into()
    } else {
        text
    }
}

fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() { "theme".into() } else { out }
}

// ---------------------------------------------------------------------
// The browser
// ---------------------------------------------------------------------

fn is_mac() -> bool {
    web_sys::window()
        .and_then(|w| w.navigator().platform().ok())
        .is_some_and(|p| p.starts_with("Mac") || p.starts_with("iP"))
}

fn media(query: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.match_media(query).ok().flatten())
        .is_some_and(|m| m.matches())
}

fn prefers_dark() -> bool {
    media("(prefers-color-scheme: dark)")
}

fn prefers_reduced_motion() -> bool {
    media("(prefers-reduced-motion: reduce)")
}

/// Sign this browser out: the site's own sign-out form, with the page's
/// CSRF value.
fn sign_out(csrf: &str) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let (Ok(form), Ok(field)) = (
        document.create_element("form"),
        document.create_element("input"),
    ) else {
        return;
    };
    let (Ok(form), Ok(field)) = (
        form.dyn_into::<web_sys::HtmlFormElement>(),
        field.dyn_into::<web_sys::HtmlInputElement>(),
    ) else {
        return;
    };
    form.set_method("post");
    form.set_action("/sign-out");
    form.set_hidden(true);
    field.set_type("hidden");
    field.set_name("csrf");
    field.set_value(csrf);
    let _ = form.append_child(&field);
    if let Some(body) = document.body() {
        let _ = body.append_child(&form);
    }
    let _ = form.submit();
}

/// Ask the browser for one file of `accept`; its name and bytes, or `None`
/// when the person closes the picker.
fn pick_file(accept: &str) -> impl std::future::Future<Output = Option<(String, Vec<u8>)>> + use<> {
    let (tx, rx) = futures::channel::oneshot::channel::<Option<web_sys::File>>();
    let input = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.create_element("input").ok())
        .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok());
    if let Some(input) = &input {
        input.set_type("file");
        input.set_accept(accept);
        let tx = std::cell::RefCell::new(Some(tx));
        let picked = input.clone();
        let change = Closure::<dyn FnMut()>::new(move || {
            if let Some(tx) = tx.borrow_mut().take() {
                let _ = tx.send(picked.files().and_then(|files| files.get(0)));
            }
        });
        input.set_onchange(Some(change.as_ref().unchecked_ref()));
        change.forget();
        input.click();
    }
    async move {
        let file = rx.await.ok().flatten()?;
        let buffer = JsFuture::from(file.array_buffer()).await.ok()?;
        Some((file.name(), js_sys::Uint8Array::new(&buffer).to_vec()))
    }
}
