//! Desktop loading and Lab cards for the shared agent visual view.
use crate::markdown::render::LinkUi;
use crate::state::EngineHandle;
use gpui::AppContext as _;
use gpui::{
    App, Context, EventEmitter, IntoElement, ParentElement, Render, SharedString, Styled, Task,
    Window, div,
};
pub use openagents_chat_view::visuals::VisualLayoutChanged;
use std::collections::HashMap;
use std::sync::Arc;

enum Load {
    Loading,
    Ready,
    Failed(SharedString),
}
pub struct VisualFrame {
    id: SharedString,
    title: SharedString,
    load: Load,
    visual: Option<gpui::Entity<openagents_chat_view::visuals::VisualFrame>>,
    link_ui: Option<LinkUi>,
    lab: Option<gpui::Entity<crate::lab_card::LabCard>>,
    lab_row: Option<gpui::AnyView>,
    _tasks: Vec<Task<()>>,
}
impl EventEmitter<VisualLayoutChanged> for VisualFrame {}
impl VisualFrame {
    pub fn new(
        id: SharedString,
        title: String,
        path: String,
        engine: Option<EngineHandle>,
        target_device_id: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut tasks = Vec::new();
        let load = match engine {
            Some(engine) => {
                tasks.push(cx.spawn(async move |this, cx| {
                    let read = |path: String| {
                        let (engine, target) = (engine.clone(), target_device_id.clone());
                        let executor = cx.background_executor().clone();
                        async move {
                            crate::attachments::read_attachment_bytes(
                                &engine,
                                &executor,
                                target.as_deref(),
                                &path,
                                None,
                            )
                            .await
                            .map(|(_, _, bytes)| bytes)
                        }
                    };
                    let script = read(path.clone())
                        .await
                        .and_then(|bytes| String::from_utf8(bytes).ok());
                    // Its files sit beside it, listed in `{id}-assets.json`
                    // (absent for a visual without any).
                    let mut files = HashMap::new();
                    if script.is_some()
                        && let Some(stem) = path.strip_suffix("-visual.luau")
                        && let Some(names) = read(format!("{stem}-assets.json"))
                            .await
                            .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
                    {
                        for name in names
                            .into_iter()
                            .filter(|name| openagents_visual::assets::valid_name(name))
                            .take(openagents_visual::assets::MAX_ASSETS)
                        {
                            if let Some(bytes) = read(format!("{stem}-asset-{name}")).await {
                                files.insert(name, Arc::<[u8]>::from(bytes));
                            }
                        }
                    }
                    let assets = openagents_visual::Assets::new(files);
                    let lab = script.as_deref().and_then(crate::lab_card::card_in_script);
                    let picture = match &lab {
                        Some(_) => {
                            let bytes = assets
                                .names()
                                .find(|name| name.starts_with("picture."))
                                .and_then(|name| assets.get(name));
                            match bytes {
                                Some(bytes) => {
                                    let bytes = bytes.to_vec();
                                    cx.background_executor()
                                        .spawn(async move { crate::lab_card::decode(bytes) })
                                        .await
                                }
                                None => None,
                            }
                        }
                        None => None,
                    };
                    let live = script
                        .as_deref()
                        .and_then(openagents_account::lab_cards::live_card_in_script);
                    if let Some(live) = live {
                        let shown = this.update(cx, |this, cx| {
                            let row = crate::lab_live::row(this.id.clone(), live, cx);
                            let shown = row.is_some();
                            if let Some(row) = row {
                                this.lab_row = Some(row.into());
                                this.load = Load::Ready;
                                cx.emit(VisualLayoutChanged);
                                cx.notify();
                            }
                            shown
                        });
                        if shown.unwrap_or(false) {
                            return;
                        }
                    }
                    let _ = this.update(cx, |this, cx| match (lab, script) {
                        (Some(card), _) => {
                            let id = this.id.clone();
                            let link_ui = this.link_ui.clone();
                            let entity = cx.new(|cx| {
                                let mut card = crate::lab_card::LabCard::new(id, card, picture, cx);
                                card.set_link_ui(link_ui);
                                card
                            });
                            this.lab = Some(entity);
                            this.load = Load::Ready;
                            cx.emit(VisualLayoutChanged);
                            cx.notify();
                        }
                        (None, Some(script)) => this.start(&script, assets, cx),
                        (None, None) => {
                            this.load = Load::Failed("Visual unavailable".into());
                            cx.notify();
                        }
                    });
                }));
                Load::Loading
            }
            None => Load::Failed("Visual unavailable".into()),
        };
        Self {
            id,
            title: title.into(),
            load,
            visual: None,
            link_ui: None,
            lab: None,
            lab_row: None,
            _tasks: tasks,
        }
    }

    pub fn set_link_ui(&mut self, link_ui: Option<LinkUi>, cx: &mut Context<Self>) {
        if let Some(lab) = &self.lab {
            let ui = link_ui.clone();
            lab.update(cx, |lab, _| lab.set_link_ui(ui));
        }
        if let Some(visual) = &self.visual {
            visual.update(cx, |visual, _| visual.set_link_ui(link_ui.clone()));
        }
        self.link_ui = link_ui;
    }

    fn start(&mut self, script: &str, assets: openagents_visual::Assets, cx: &mut Context<Self>) {
        use crate::settings::widgets;
        let controls = openagents_chat_view::visuals::Controls {
            button: |theme, primary| {
                widgets::action_button(
                    theme,
                    if primary {
                        widgets::ActionTone::Solid
                    } else {
                        widgets::ActionTone::Filled
                    },
                )
            },
            toggle: |theme, on, key| widgets::toggle_switch(theme, on, key),
            tab_selection: |window, key, selected, reduced| {
                widgets::tab_selection_t(window, key, selected, reduced)
            },
            tab: |theme, selected, t, id, key| widgets::section_tab(theme, selected, t, id, key),
        };
        let visual = cx.new(|cx| {
            let mut visual = openagents_chat_view::visuals::VisualFrame::new(
                self.id.clone(),
                self.title.to_string(),
                script,
                assets,
                controls,
                cx,
            );
            visual.set_link_ui(self.link_ui.clone());
            visual
        });
        cx.subscribe(&visual, |_, _, _: &VisualLayoutChanged, cx| {
            cx.emit(VisualLayoutChanged);
            cx.notify();
        })
        .detach();
        self.visual = Some(visual);
        self.load = Load::Ready;
        cx.emit(VisualLayoutChanged);
        cx.notify();
    }
}
impl Render for VisualFrame {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut frame = div().w_full();
        if let Some(row) = &self.lab_row {
            return frame.child(row.clone());
        }
        if let Some(lab) = &self.lab {
            return frame.child(lab.clone());
        }
        if let Some(visual) = &self.visual {
            return frame.child(visual.clone());
        }
        frame = frame.child(match &self.load {
            Load::Failed(message) => message.clone(),
            _ => "Loading visual…".into(),
        });
        frame
    }
}
