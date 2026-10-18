//! Settings forms and confirmation state remain in Rust on every phone shell.
use super::preferences::*;
use coder_ui::android::AndroidView;
use rust_native::{Activation, Element, FieldChange, Node, TextRole, View};
use serde::Serialize;

#[derive(Clone, Debug, Default, uniffi::Record)]
pub struct SettingsCapabilities {
    pub appearance: bool,
    pub wallpaper_picker: bool,
    pub pairing: bool,
    pub cloud: bool,
    pub archive: bool,
    pub project_sharing: bool,
    pub notification_settings: bool,
    pub share_chooser: bool,
}
#[derive(Clone)]
pub(super) struct Device {
    pub id: String,
    pub name: String,
    pub online: bool,
}
pub(super) struct Input {
    pub generation: u64,
    pub signed_in: bool,
    pub name: String,
    pub devices: Vec<Device>,
    pub cloud_running: Option<bool>,
    pub caps: SettingsCapabilities,
    pub version: String,
}
#[derive(Clone, Debug, Serialize)]
enum Intent {
    About,
    Close,
    ShareApp,
    Pair,
    Archive,
    Sharing,
    Notifications,
    Appearance(SettingsAppearance),
    ChooseWallpaper,
    RemoveWallpaper,
    WallpaperEffect(SettingsWallpaperEffect),
    ResetAppearance,
    Rename(String),
    RenameValue,
    SaveRename,
    Delete,
    ConfirmDelete,
    ConfirmCredit,
    SignOut,
    Cloud(bool),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Operation {
    pub serial: u64,
    pub generation: u64,
}
#[derive(Clone)]
pub(super) enum Action {
    Save(SettingsPreferences),
    Delete(bool),
    Cloud(bool),
    Rename { id: String, name: String },
    SignOut,
    Pair,
    Archive,
    Sharing,
    Notifications,
    ChooseWallpaper { ticket: String },
    ShareApp,
}
enum Dialog {
    None,
    About,
    Rename { id: String, value: String },
    Delete,
    Credit(String),
    Notice(String),
}
pub(super) struct Model {
    input: Input,
    pub prefs: SettingsPreferences,
    prefs_available: bool,
    dialog: Dialog,
    busy: Option<Operation>,
    serial: u64,
    revision: u64,
    instance: String,
    wallpaper_ticket: Option<String>,
    bridge: AndroidView<Intent>,
}
impl Model {
    pub fn new(input: Input, prefs: SettingsPreferences, read_error: Option<String>) -> Self {
        let available = read_error.is_none();
        Self {
            input,
            prefs,
            prefs_available: available,
            dialog: read_error.map(Dialog::Notice).unwrap_or(Dialog::None),
            busy: None,
            serial: 0,
            revision: 0,
            instance: format!("settings-{}", uuid::Uuid::new_v4()),
            wallpaper_ticket: None,
            bridge: AndroidView::new(),
        }
    }
    pub fn accepts(&self, operation: &Operation) -> bool {
        self.busy.as_ref() == Some(operation) && self.input.generation == operation.generation
    }
    pub fn close(&mut self) {
        self.busy = None;
        self.wallpaper_ticket = None;
        self.dialog = Dialog::None;
        self.bridge.dispose();
    }
    pub fn update(&mut self, input: Input) -> Result<(), String> {
        if input.generation != self.input.generation {
            self.close();
            return Err("Your account changed. Open Settings again to continue.".into());
        }
        self.input = input;
        Ok(())
    }
    pub fn prepare(&mut self) -> Result<Vec<u8>, String> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or("Open Settings again to continue.")?;
        let view = View::new_v3(
            &self.instance,
            self.revision,
            node(
                "settings",
                Element::List {
                    label: "Settings".into(),
                    children: self.children(),
                },
            ),
        );
        self.bridge
            .prepare(
                view.validate()
                    .map_err(|_| "Couldn't show Settings. Try again.")?,
            )
            .map_err(|_| "Couldn't show Settings. Try again.".into())
    }
    pub fn acknowledge(&mut self, instance: &str, revision: u64) -> Result<(), String> {
        self.bridge
            .acknowledge(instance, revision)
            .map_err(|_| "Open Settings again to continue.".into())
    }
    fn children(&self) -> Vec<Node<Intent>> {
        let enabled = self.busy.is_none();
        let mut children = vec![
            text("heading", "Settings", true),
            text("account-name", &self.input.name, false),
        ];
        if self.input.signed_in {
            if self.input.caps.pairing {
                children.push(button("pair", "Connect a computer", enabled, Intent::Pair));
            }
            for (at, device) in self.input.devices.iter().enumerate() {
                children.push(button(
                    &format!("device-{at}"),
                    &device.name,
                    enabled,
                    Intent::Rename(device.id.clone()),
                ));
                children.push(text(
                    &format!("device-status-{at}"),
                    if device.online { "Online" } else { "Offline" },
                    false,
                ));
            }
            if self.input.caps.cloud {
                if let Some(running) = self.input.cloud_running {
                    children.push(button(
                        "cloud",
                        if running {
                            "Stop OpenAgents Cloud"
                        } else {
                            "Start OpenAgents Cloud"
                        },
                        enabled,
                        Intent::Cloud(!running),
                    ));
                }
            }
            if self.input.caps.project_sharing {
                children.push(button(
                    "sharing",
                    "Project sharing",
                    enabled,
                    Intent::Sharing,
                ));
            }
            if self.input.caps.archive {
                children.push(button(
                    "archive",
                    "Archived sessions",
                    enabled,
                    Intent::Archive,
                ));
            }
        }
        if self.input.caps.appearance {
            children.push(text("appearance-title", "Appearance", true));
            if self.prefs_available {
                for (key, label, appearance) in [
                    ("system", "System", SettingsAppearance::System),
                    ("light", "Light", SettingsAppearance::Light),
                    ("dark", "Dark", SettingsAppearance::Dark),
                ] {
                    children.push(node(
                        key,
                        Element::Choice {
                            label: label.into(),
                            selected: self.prefs.appearance == appearance,
                            enabled,
                            intent: Intent::Appearance(appearance),
                            children: vec![],
                        },
                    ));
                }
            } else {
                children.push(button(
                    "reset-appearance",
                    "Reset appearance",
                    enabled,
                    Intent::ResetAppearance,
                ));
            }
        }
        if self.input.caps.wallpaper_picker && self.prefs_available {
            children.push(button(
                "wallpaper",
                if self.prefs.wallpaper.is_some() {
                    "Change wallpaper"
                } else {
                    "Choose wallpaper"
                },
                enabled,
                Intent::ChooseWallpaper,
            ));
            if let Some(wallpaper) = &self.prefs.wallpaper {
                children.push(text("wallpaper-name", &wallpaper.name, false));
                for (key, label, effect) in [
                    ("none", "Original", SettingsWallpaperEffect::None),
                    ("dither", "Dither", SettingsWallpaperEffect::Dither),
                    ("ascii", "Text", SettingsWallpaperEffect::Ascii),
                    ("halftone", "Halftone", SettingsWallpaperEffect::Halftone),
                    ("scanlines", "Scanlines", SettingsWallpaperEffect::Scanlines),
                ] {
                    children.push(node(
                        &format!("wallpaper-{key}"),
                        Element::Choice {
                            label: label.into(),
                            selected: wallpaper.effect == effect,
                            enabled,
                            intent: Intent::WallpaperEffect(effect),
                            children: vec![],
                        },
                    ));
                }
                children.push(button(
                    "remove-wallpaper",
                    "Remove wallpaper",
                    enabled,
                    Intent::RemoveWallpaper,
                ));
            }
        }
        if self.input.caps.notification_settings {
            children.push(button(
                "notifications",
                "Notification settings",
                enabled,
                Intent::Notifications,
            ));
        }
        children.push(button("about", "About", enabled, Intent::About));
        if self.input.signed_in {
            children.push(button("sign-out", "Sign out", enabled, Intent::SignOut));
            children.push(button("delete", "Delete account", enabled, Intent::Delete));
        }
        if !enabled {
            children.push(node(
                "working",
                Element::Working {
                    label: "Saving…".into(),
                },
            ));
        }
        let mut body = vec![];
        let title = match &self.dialog {
            Dialog::None => None,
            Dialog::About => {
                body.extend([
                    text("brand", openagents_brand::NAME, true),
                    text("version", &self.input.version, false),
                    text("copyright", openagents_brand::COPYRIGHT, false),
                    text("license", "Licensed under the Apache License 2.0.", false),
                    text(
                        "third-party",
                        include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../../THIRD_PARTY_NOTICES.md"
                        )),
                        false,
                    ),
                ]);
                if self.input.caps.share_chooser {
                    body.push(button(
                        "share-app",
                        "Share OpenAgents",
                        enabled,
                        Intent::ShareApp,
                    ));
                }
                Some("About")
            }
            Dialog::Rename { value, .. } => {
                body.push(node(
                    "device-name",
                    Element::Field {
                        label: "Device name".into(),
                        value: value.clone(),
                        placeholder: "Device name".into(),
                        secret: false,
                        multiline: false,
                        enabled,
                        max_bytes: 128,
                        on_change: Intent::RenameValue,
                    },
                ));
                body.push(button("save-name", "Save", enabled, Intent::SaveRename));
                Some("Rename computer")
            }
            Dialog::Delete => {
                body.push(text(
                    "delete-warning",
                    openagents_account::DELETE_WARNING,
                    false,
                ));
                body.push(button(
                    "confirm-delete",
                    "Delete account",
                    enabled,
                    Intent::ConfirmDelete,
                ));
                Some("Delete your account?")
            }
            Dialog::Credit(message) => {
                body.push(text("credit-warning", message, false));
                body.push(button(
                    "confirm-credit",
                    "Give up and delete",
                    enabled,
                    Intent::ConfirmCredit,
                ));
                Some("Give up your credit?")
            }
            Dialog::Notice(message) => {
                body.push(text("notice", message, false));
                Some("Settings")
            }
        };
        if let Some(title) = title {
            body.push(button("close-dialog", "Close", enabled, Intent::Close));
            children.push(node(
                "dialog",
                Element::Dialog {
                    label: title.into(),
                    open: true,
                    on_close: Intent::Close,
                    children: body,
                },
            ));
        }
        children
    }
    pub fn change_field(&mut self, event: FieldChange) -> Result<(), String> {
        let intent = self
            .bridge
            .change_field(&event)
            .map_err(|_| "Open Settings again to continue.")?;
        if self.busy.is_some() || !matches!(intent, Intent::RenameValue) {
            return Err("Wait for the current change to finish.".into());
        }
        let Dialog::Rename { value, .. } = &mut self.dialog else {
            return Err("Open the computer again to rename it.".into());
        };
        *value = event.value;
        Ok(())
    }
    pub fn activate(&mut self, event: Activation) -> Result<Option<(Operation, Action)>, String> {
        let intent = self
            .bridge
            .activate(&event)
            .map_err(|_| "Open Settings again to continue.")?
            .clone();
        if self.busy.is_some() {
            return Err("Wait for the current change to finish.".into());
        }
        let action = match intent {
            Intent::Close => {
                self.dialog = Dialog::None;
                return Ok(None);
            }
            Intent::About => {
                self.dialog = Dialog::About;
                return Ok(None);
            }
            Intent::Delete if self.input.signed_in => {
                self.dialog = Dialog::Delete;
                return Ok(None);
            }
            Intent::ConfirmDelete if matches!(self.dialog, Dialog::Delete) => Action::Delete(false),
            Intent::ConfirmCredit if matches!(self.dialog, Dialog::Credit(_)) => {
                Action::Delete(true)
            }
            Intent::Rename(id) if self.input.devices.iter().any(|device| device.id == id) => {
                let value = self
                    .input
                    .devices
                    .iter()
                    .find(|device| device.id == id)
                    .unwrap()
                    .name
                    .clone();
                self.dialog = Dialog::Rename { id, value };
                return Ok(None);
            }
            Intent::SaveRename => match &self.dialog {
                Dialog::Rename { id, value } => Action::Rename {
                    id: id.clone(),
                    name: value.clone(),
                },
                _ => return Err("Open the computer again to rename it.".into()),
            },
            Intent::Appearance(appearance) => {
                let mut prefs = self.prefs.clone();
                prefs.appearance = appearance;
                Action::Save(prefs)
            }
            Intent::RemoveWallpaper => {
                let mut prefs = self.prefs.clone();
                prefs.wallpaper = None;
                Action::Save(prefs)
            }
            Intent::WallpaperEffect(effect) => {
                let mut prefs = self.prefs.clone();
                let Some(wallpaper) = &mut prefs.wallpaper else {
                    return Err("Choose a wallpaper first.".into());
                };
                wallpaper.effect = effect;
                Action::Save(prefs)
            }
            Intent::ResetAppearance => Action::Save(Default::default()),
            Intent::ChooseWallpaper => {
                let ticket = uuid::Uuid::new_v4().to_string();
                self.wallpaper_ticket = Some(ticket.clone());
                Action::ChooseWallpaper { ticket }
            }
            Intent::SignOut if self.input.signed_in => Action::SignOut,
            Intent::Cloud(start) if self.input.signed_in && self.input.cloud_running.is_some() => {
                Action::Cloud(start)
            }
            Intent::Pair => Action::Pair,
            Intent::Archive => Action::Archive,
            Intent::Sharing => Action::Sharing,
            Intent::Notifications => Action::Notifications,
            Intent::ShareApp => Action::ShareApp,
            _ => return Err("Open Settings again to continue.".into()),
        };
        Ok(Some((self.begin(), action)))
    }
    fn begin(&mut self) -> Operation {
        self.serial += 1;
        let operation = Operation {
            serial: self.serial,
            generation: self.input.generation,
        };
        self.busy = Some(operation.clone());
        operation
    }
    pub fn wallpaper(
        &mut self,
        ticket: &str,
        uri: String,
        name: String,
        permission_saved: bool,
    ) -> Result<(Operation, Action), String> {
        if self.wallpaper_ticket.as_deref() != Some(ticket) || self.busy.is_some() {
            return Err("Choose a wallpaper again to continue.".into());
        }
        self.wallpaper_ticket = None;
        if !permission_saved {
            let message = "Couldn't keep this wallpaper. Choose another image.".to_owned();
            self.dialog = Dialog::Notice(message.clone());
            return Err(message);
        }
        let mut prefs = self.prefs.clone();
        prefs.wallpaper = Some(SettingsWallpaper {
            uri,
            name,
            effect: Default::default(),
        });
        Ok((self.begin(), Action::Save(prefs)))
    }
    pub fn complete(
        &mut self,
        operation: &Operation,
        result: Result<Option<SettingsPreferences>, String>,
    ) -> bool {
        if self.busy.as_ref() != Some(operation) || operation.generation != self.input.generation {
            return false;
        }
        self.busy = None;
        match result {
            Ok(Some(prefs)) => {
                self.prefs = prefs;
                self.prefs_available = true;
                self.dialog = Dialog::None;
            }
            Ok(None) => self.dialog = Dialog::None,
            Err(message) => self.dialog = Dialog::Notice(message),
        }
        true
    }
    pub fn deletion(
        &mut self,
        operation: &Operation,
        result: openagents_account::Deletion,
    ) -> bool {
        if self.busy.as_ref() != Some(operation) || operation.generation != self.input.generation {
            return false;
        }
        self.busy = None;
        self.dialog = match result {
            openagents_account::Deletion::PaidCredit => {
                Dialog::Credit(openagents_account::PAID_CREDIT_WARNING.into())
            }
            openagents_account::Deletion::SharedTeam => {
                Dialog::Notice(openagents_account::SHARED_TEAM_NOTICE.into())
            }
            openagents_account::Deletion::Failed(message) => Dialog::Notice(message),
            openagents_account::Deletion::Deleted => Dialog::None,
        };
        true
    }
}
fn node(key: &str, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Default::default(),
        element,
    }
}
fn text(key: &str, value: &str, heading: bool) -> Node<Intent> {
    node(
        key,
        Element::Text {
            value: value.into(),
            role: if heading {
                TextRole::Heading
            } else {
                TextRole::Body
            },
        },
    )
}
fn button(key: &str, label: &str, enabled: bool, intent: Intent) -> Node<Intent> {
    node(
        key,
        Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            shortcut: None,
            intent,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        Input {
            generation: 7,
            signed_in: true,
            name: "Test account".into(),
            devices: vec![],
            cloud_running: None,
            caps: SettingsCapabilities {
                appearance: true,
                ..Default::default()
            },
            version: "Test build".into(),
        }
    }
    fn model() -> Model {
        Model::new(input(), Default::default(), None)
    }
    fn shown(model: &mut Model) -> serde_json::Value {
        let bytes = model.prepare().unwrap();
        let view: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        model
            .acknowledge(
                view["instance"].as_str().unwrap(),
                view["revision"].as_u64().unwrap(),
            )
            .unwrap();
        view
    }
    fn event(view: &serde_json::Value, key: &str) -> Activation {
        Activation {
            instance: view["instance"].as_str().unwrap().into(),
            revision: view["revision"].as_u64().unwrap(),
            node: key.into(),
        }
    }
    #[test]
    fn deletion_requires_current_mounted_warning_and_a_second_paid_credit_confirmation() {
        let mut model = model();
        let raw: serde_json::Value = serde_json::from_slice(&model.prepare().unwrap()).unwrap();
        assert!(model.activate(event(&raw, "delete")).is_err());
        let view = shown(&mut model);
        assert!(model.activate(event(&view, "delete")).unwrap().is_none());
        let warning = shown(&mut model);
        let (operation, action) = model
            .activate(event(&warning, "confirm-delete"))
            .unwrap()
            .unwrap();
        assert!(matches!(action, Action::Delete(false)));
        assert!(model.activate(event(&warning, "confirm-delete")).is_err());
        assert!(model.deletion(&operation, openagents_account::Deletion::PaidCredit));
        let credit = shown(&mut model);
        assert!(model.activate(event(&warning, "confirm-delete")).is_err());
        let (operation, action) = model
            .activate(event(&credit, "confirm-credit"))
            .unwrap()
            .unwrap();
        assert!(matches!(action, Action::Delete(true)));
        model.deletion(&operation, openagents_account::Deletion::SharedTeam);
        let refusal = shown(&mut model);
        let text = refusal.to_string();
        assert!(text.contains(openagents_account::SHARED_TEAM_NOTICE));
        assert!(!text.contains("confirm-credit"));
        assert!(model.activate(event(&refusal, "confirm-credit")).is_err());
    }
    #[test]
    fn cancel_failure_restart_and_account_change_never_reuse_a_forfeit_confirmation() {
        let mut model = model();
        let view = shown(&mut model);
        model.activate(event(&view, "delete")).unwrap();
        let warning = shown(&mut model);
        model.activate(event(&warning, "close-dialog")).unwrap();
        let main = shown(&mut model);
        assert!(model.activate(event(&main, "confirm-delete")).is_err());
        model.activate(event(&main, "delete")).unwrap();
        let warning = shown(&mut model);
        let (operation, _) = model
            .activate(event(&warning, "confirm-delete"))
            .unwrap()
            .unwrap();
        model.deletion(
            &operation,
            openagents_account::Deletion::Failed("Check your connection and try again.".into()),
        );
        assert!(!shown(&mut model).to_string().contains("confirm-credit"));
        let mut restarted = Model::new(input(), model.prefs.clone(), None);
        assert!(!shown(&mut restarted).to_string().contains("confirm-delete"));
        let mut changed = input();
        changed.generation += 1;
        assert!(model.update(changed).is_err());
        assert!(!model.deletion(&operation, openagents_account::Deletion::PaidCredit));
        assert!(model.prepare().is_err());
    }
    #[test]
    fn unavailable_controls_stay_absent_and_preferences_change_only_after_save() {
        let mut model = model();
        let view = shown(&mut model);
        for key in [
            "pair",
            "sharing",
            "archive",
            "cloud",
            "notifications",
            "wallpaper",
        ] {
            assert!(model.activate(event(&view, key)).is_err());
        }
        let (operation, action) = model.activate(event(&view, "dark")).unwrap().unwrap();
        let Action::Save(prefs) = action else {
            panic!("expected preferences");
        };
        assert_eq!(model.prefs.appearance, SettingsAppearance::System);
        model.complete(
            &operation,
            Err("Couldn't save your settings. Try again.".into()),
        );
        assert_eq!(model.prefs.appearance, SettingsAppearance::System);
        let notice = shown(&mut model);
        model.activate(event(&notice, "close-dialog")).unwrap();
        let view = shown(&mut model);
        let (operation, _) = model.activate(event(&view, "dark")).unwrap().unwrap();
        model.complete(&operation, Ok(Some(prefs)));
        assert_eq!(model.prefs.appearance, SettingsAppearance::Dark);
    }
    #[test]
    fn wallpaper_results_need_the_live_single_use_picker_ticket_and_saved_permission() {
        let mut input = input();
        input.caps.wallpaper_picker = true;
        let mut model = Model::new(input, Default::default(), None);
        let view = shown(&mut model);
        let (operation, action) = model.activate(event(&view, "wallpaper")).unwrap().unwrap();
        let Action::ChooseWallpaper { ticket } = action else {
            panic!("expected picker");
        };
        model.complete(&operation, Ok(None));
        assert!(
            model
                .wallpaper("other", "content://fixture/1".into(), "Image".into(), true)
                .is_err()
        );
        assert!(
            model
                .wallpaper(&ticket, "content://fixture/1".into(), "Image".into(), false)
                .is_err()
        );
        assert!(
            model
                .wallpaper(&ticket, "content://fixture/1".into(), "Image".into(), true)
                .is_err()
        );
        assert!(model.prefs.wallpaper.is_none());
    }
    #[test]
    fn settings_product_copy_passes_the_shared_guard() {
        let mut model = model();
        let view = shown(&mut model);
        fn inspect(value: &serde_json::Value) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, value) in map {
                        if matches!(key.as_str(), "label" | "placeholder") {
                            if let Some(copy) = value.as_str() {
                                assert!(oa_copy::violations(copy, &[]).is_empty(), "{copy}");
                            }
                        }
                        inspect(value);
                    }
                }
                serde_json::Value::Array(values) => values.iter().for_each(inspect),
                _ => {}
            }
        }
        inspect(&view);
        model.activate(event(&view, "about")).unwrap();
        inspect(&shown(&mut model));
        for copy in [
            openagents_account::DELETE_WARNING,
            openagents_account::PAID_CREDIT_WARNING,
            openagents_account::SHARED_TEAM_NOTICE,
        ] {
            assert!(oa_copy::violations(copy, &[]).is_empty());
        }
    }
}
