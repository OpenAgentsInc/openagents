//! Add a project on a computer from the web (#12318), as the desktop's
//! "Add a project" does: browse the computer's folders (`ListFolders`) or
//! search its home folder (`SearchHomeFolders`), pick one, and the computer
//! adds it as a project (`Mutate` `createSpace`, the one workspace change a
//! browser may ask for). The new project is then the session's project.

use gpui::{
    AnyElement, ClickEvent, Context, Entity, Focusable as _, IntoElement, ParentElement,
    SharedString, Styled, Window, div, prelude::*, px,
};
use openagents_proto::{FolderEntry, FolderListing, HomeFolderMatch};
use openagents_web_chat_view::composer::{Submitted, TextInput};
use openagents_web_chat_view::transcript::button;
use openagents_web_chat_view::{svg, theme};
use openagents_web_client::computer;
use serde_json::json;

use super::ChatApp;
use super::new_session::Picker;
use crate::model::{join, parent};

const LIST_FOLDERS: &str = "ListFolders";
const SEARCH_HOME_FOLDERS: &str = "SearchHomeFolders";
const MUTATE: &str = "Mutate";

/// The folder browser's place.
#[derive(Default)]
pub(super) struct Folder {
    pub path: String,
    /// The folder is a git repository.
    pub repo: bool,
    pub entries: Vec<FolderEntry>,
    /// Search results, when the search field has words in it.
    pub matches: Option<Vec<HomeFolderMatch>>,
    pub search: Option<Entity<TextInput>>,
}

impl ChatApp {
    async fn folder_call(
        client: &openagents_web_client::web::Client,
        device: &str,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let computer = computer::connect(client, device, || {}).await?;
        computer.call(method, params).await
    }

    /// Opens the folder browser at `path` (the home folder when `None`).
    pub(super) fn browse(
        &mut self,
        path: Option<String>,
        repo: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(device) = self.target.computer.clone() else {
            return;
        };
        self.target.open = Some(Picker::Folder);
        let search = match self.target.folder.as_ref().and_then(|f| f.search.clone()) {
            Some(search) => search,
            None => {
                let field = cx.new(|cx| TextInput::new("Search your home folder", cx));
                cx.subscribe(&field, |this, _, event: &Submitted, cx| {
                    this.search_folders(event.0.trim().to_owned(), cx)
                })
                .detach();
                field
            }
        };
        window.focus(&search.focus_handle(cx), cx);
        let folder = self.target.folder.get_or_insert_with(Folder::default);
        folder.search = Some(search);
        folder.matches = None;
        folder.repo = repo;
        self.target.loading = Some("Reading this computer's folders…");
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let listing =
                Self::folder_call(&client, &device, LIST_FOLDERS, json!({ "path": path }))
                    .await
                    .and_then(|v| Ok(serde_json::from_value::<FolderListing>(v)?));
            this.update(cx, |this, cx| {
                this.target.loading = None;
                match listing {
                    Ok(listing) => {
                        if let Some(folder) = &mut this.target.folder {
                            folder.path = listing.path;
                            folder.entries =
                                listing.entries.into_iter().filter(|e| e.is_dir).collect();
                        }
                    }
                    Err(error) => this.notice = Some(error.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn search_folders(&mut self, query: String, cx: &mut Context<Self>) {
        let Some(device) = self.target.computer.clone() else {
            return;
        };
        if query.is_empty() {
            if let Some(folder) = &mut self.target.folder {
                folder.matches = None;
            }
            cx.notify();
            return;
        }
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let found = Self::folder_call(
                &client,
                &device,
                SEARCH_HOME_FOLDERS,
                json!({ "query": query, "limit": 30 }),
            )
            .await;
            this.update(cx, |this, cx| {
                let matches = found
                    .ok()
                    .and_then(|v| {
                        serde_json::from_value::<Vec<HomeFolderMatch>>(v["matches"].clone())
                            .ok()
                            .or_else(|| serde_json::from_value(v).ok())
                    })
                    .unwrap_or_default();
                if let Some(folder) = &mut this.target.folder {
                    folder.matches = Some(matches);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Asks the computer to add `path` as a project, then picks it.
    fn add_project(&mut self, path: String, repo: bool, cx: &mut Context<Self>) {
        let (Some(device), Some(registry)) = (self.target.computer.clone(), self.registry.clone())
        else {
            return;
        };
        if let Some(existing) = registry.read(|r| {
            r.spaces()
                .into_iter()
                .find(|s| s.device_id == device && s.path == path)
        }) {
            self.target.folder = None;
            self.choose_project(Some(existing.id), cx);
            return;
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.target.loading = Some("Adding the project…");
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let added = Self::folder_call(
                &client,
                &device,
                MUTATE,
                json!({"op": "createSpace", "spaceId": id, "deviceId": device,
                    "path": path, "gitDetected": repo}),
            )
            .await;
            // The computer writes the project's row; wait for it here.
            if added.is_ok() {
                for _ in 0..50 {
                    if registry.read(|r| r.spaces().iter().any(|s| s.id == id)) {
                        break;
                    }
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(100))
                        .await;
                }
            }
            this.update(cx, |this, cx| {
                this.target.loading = None;
                match added {
                    Ok(_) => {
                        this.target.folder = None;
                        this.choose_project(Some(id.clone()), cx);
                    }
                    Err(error) => this.notice = Some(error.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// The folder browser's rows: search, the folder's subfolders (or the
    /// search's matches), up, and "Add this folder".
    pub(super) fn folder_choices(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(folder) = &self.target.folder else {
            return Vec::new();
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        if let Some(search) = &folder.search {
            rows.push(
                div()
                    .mx(px(4.))
                    .mb(px(4.))
                    .px(px(8.))
                    .py(px(5.))
                    .rounded(px(7.))
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::input_bg())
                    .text_size(px(13.))
                    .child(search.clone())
                    .into_any_element(),
            );
        }
        if let Some(matches) = &folder.matches {
            if matches.is_empty() {
                rows.push(line("No folder has that name."));
            }
            for (ix, found) in matches.iter().enumerate() {
                let (path, repo) = (found.path.clone(), found.is_repo);
                rows.push(
                    row(format!("folder-match-{ix}"), found.relative.clone(), repo)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.browse(Some(path.clone()), repo, window, cx)
                        }))
                        .into_any_element(),
                );
            }
            return rows;
        }
        rows.push(line(&folder.path));
        if let Some(up) = parent(&folder.path) {
            rows.push(
                row("folder-up".into(), "..".into(), false)
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.browse(Some(up.clone()), false, window, cx)
                    }))
                    .into_any_element(),
            );
        }
        for (ix, entry) in folder.entries.iter().enumerate() {
            let (path, repo) = (join(&folder.path, &entry.name), entry.is_repo);
            rows.push(
                row(format!("folder-{ix}"), entry.name.clone(), repo)
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.browse(Some(path.clone()), repo, window, cx)
                    }))
                    .into_any_element(),
            );
        }
        if !folder.path.is_empty() {
            let (path, repo) = (folder.path.clone(), folder.repo);
            rows.push(
                div()
                    .p(px(6.))
                    .child(
                        button("folder-add", "Add this folder", true).on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                this.add_project(path.clone(), repo, cx)
                            },
                        )),
                    )
                    .into_any_element(),
            );
        }
        rows
    }
}

fn line(text: &str) -> AnyElement {
    div()
        .px(px(8.))
        .py(px(4.))
        .text_size(px(12.))
        .text_color(theme::text_faint())
        .truncate()
        .child(text.to_owned())
        .into_any_element()
}

fn row(id: String, label: String, repo: bool) -> gpui::Stateful<gpui::Div> {
    div().id(SharedString::from(id)).child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(8.))
            .h(px(28.))
            .rounded(px(7.))
            .text_size(px(13.))
            .cursor_pointer()
            .hover(|s| s.bg(theme::element_hover()))
            .child(
                svg()
                    .path(if repo {
                        "icons/git-branch.svg"
                    } else {
                        "icons/folder.svg"
                    })
                    .size(px(13.))
                    .text_color(theme::text_muted()),
            )
            .child(div().truncate().child(label)),
    )
}
