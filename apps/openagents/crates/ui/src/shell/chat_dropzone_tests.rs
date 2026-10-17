use super::*;
use gpui::{AppContext, TestAppContext, VisualTestContext};

struct DropHost {
    shell: Entity<Shell>,
    full_shell: bool,
    _data_dir: tempfile::TempDir,
}

impl Render for DropHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.full_shell {
            return div()
                .size_full()
                .child(self.shell.clone())
                .into_any_element();
        }
        self.shell.update(cx, |shell, cx| {
            let main = shell.render_main(window, 400., 400., cx);
            let right = shell.render_right_pane(window, cx);
            // Tree and search rows stamp the explorer's current origin; the
            // stale source models a drag that outlived its workspace.
            let origin = shell
                .files
                .get(&shell.panel_key(cx))
                .and_then(|files| files.read(cx).interaction_origin(cx));
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(div().h(px(40.)).child(shell.render_right_tab_strip(cx)))
                // Supply the tree/search payload without needing an engine
                // to enumerate a checkout. Targets and file tabs are real UI.
                .child(
                    div().h(px(24.)).flex().children(
                        [
                            ("file", "src/tree.rs", false, origin.clone()),
                            ("directory", "src", true, origin),
                            ("stale", "src/stale.rs", false, None),
                        ]
                        .into_iter()
                        .map(|(id, path, directory, origin)| {
                            div()
                                .id(id)
                                .debug_selector(move || id.into())
                                .w(px(100.))
                                .h_full()
                                .on_drag(
                                    WorkspacePathDrag::new(path.into(), directory).with_origin(
                                        origin,
                                        crate::files::WorkspacePathSource::Tree,
                                        None,
                                    ),
                                    |payload, _, _, cx| {
                                        crate::files::workspace_path_drag_ghost(payload, cx)
                                    },
                                )
                                .child(id)
                        }),
                    ),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .child(div().w(px(400.)).h_full().child(main))
                        .child(right),
                )
                .into_any_element()
        })
    }
}

fn setup_with_shell(
    cx: &mut TestAppContext,
    full_shell: bool,
) -> (Entity<Shell>, &mut VisualTestContext) {
    let dir = tempfile::tempdir().unwrap();
    cx.update(|cx| {
        gpui_base::init(cx);
        cx.set_global(Theme::default());
        crate::app_menus::init(cx);
        settings::init(UiSettings::default(), dir.path(), cx);
        crate::history::init(
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            cx,
        );
    });
    let (host, cx) = cx.add_window_view(|_, cx| {
        let shell = cx.new(|cx| {
            let state = cx.new(|_| AppState::new());
            let mut shell = Shell::new(
                state,
                EngineBootConfig {
                    data_dir: dir.path().into(),
                    ipc_port: 0,
                    edge_url: "http://127.0.0.1:1".into(),
                    edge_token: None,
                    org_id: None,
                    workos_client_id: None,
                    default_harness: openagents_proto::HarnessId::Mock,
                },
                cx,
            );
            shell.route = Route::Chat;
            // This fixture has no engine. Keep state notifications from
            // invoking the shell's disconnect cleanup of all side chats.
            shell._state_observation = cx.observe(&shell.state, |_, _, _| {});
            shell.state.update(cx, |state, _| {
                state.workspace_scope = Some(WorkspaceScope::Local);
                state.connection = ConnectionStatus::Ready;
                state.local_device_id = Some("local".into());
                state.no_project = true;
                state.chats = vec![
                    serde_json::from_value(serde_json::json!({
                        "id": "parent", "deviceId": "local", "archived": false,
                        "createdAt": Utc::now(),
                    }))
                    .unwrap(),
                ];
                state.selected_chat = Some("parent".into());
            });
            shell.active_chat = "parent".into();
            shell.splash = SplashPhase::Gone;
            shell.reduced_motion = true;
            shell
        });
        DropHost {
            shell,
            full_shell,
            _data_dir: dir,
        }
    });
    let shell = host.read_with(cx, |host, _| host.shell.clone());
    cx.update(|window, cx| window.draw(cx).clear());
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.add_file_surface("src/tab.rs".into(), window, cx);
            let explorer = cx.new(|cx| {
                crate::files::FilesSurface::new_explorer(
                    shell.state.clone(),
                    shell.active_chat.clone(),
                    false,
                    cx,
                )
            });
            shell.files.insert(shell.panel_key(cx), explorer);
            shell.right_tween = None;
        });
    });
    cx.update(|window, cx| window.draw(cx).clear());
    (shell, cx)
}

fn text(composer: &Entity<Composer>, cx: &App) -> String {
    composer.read(cx).input.read(cx).text().to_owned()
}

fn external_files() -> (tempfile::TempDir, gpui::ExternalPaths) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("notes.zip"),
        b"PK\x03\x04 not really zipped",
    )
    .unwrap();
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbImage::new(1, 1)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    std::fs::write(dir.path().join("shot.png"), png.into_inner()).unwrap();
    std::fs::create_dir(dir.path().join("folder")).unwrap();
    // Sparse, so the test never writes 24 MB to disk.
    std::fs::File::create(dir.path().join("huge.bin"))
        .unwrap()
        .set_len(crate::attachments::MAX_ATTACHMENT_BYTES + 1)
        .unwrap();
    let paths = ["notes.zip", "shot.png", "folder", "huge.bin"]
        .into_iter()
        .map(|name| dir.path().join(name))
        .collect();
    (dir, gpui::ExternalPaths(paths))
}

/// Drag files in from outside the app and release them at `to` the way the
/// platform reports it: enter, hover, then drop.
fn drop_external(cx: &mut VisualTestContext, paths: gpui::ExternalPaths, to: Point<Pixels>) {
    cx.simulate_event(gpui::FileDropEvent::Entered {
        position: to,
        paths,
    });
    cx.update(|window, cx| {
        assert!(cx.has_active_drag());
        window.draw(cx).clear();
    });
    cx.simulate_event(gpui::FileDropEvent::Pending { position: to });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.simulate_event(gpui::FileDropEvent::Submit { position: to });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert!(!cx.has_active_drag());
        window.draw(cx).clear();
    });
}

fn staged_names(composer: &Entity<Composer>, cx: &App) -> Vec<String> {
    composer
        .read(cx)
        .staged()
        .iter()
        .map(|att| att.name.clone())
        .collect()
}

/// A mixed drop from outside the app lands in the conversation's composer:
/// the archive and the image are staged with chips in that draft, and the
/// folder and the oversize file are both named in its error notice.
#[gpui::test]
fn external_file_drops_stage_files_and_refuse_folders_and_oversize_files(cx: &mut TestAppContext) {
    use openagents_proto::attachment_mentions::attachment_mention_link;
    let (shell, cx) = setup_with_shell(cx, true);
    let (_files, paths) = external_files();
    let to = cx.debug_bounds("chat-dropzone").unwrap().center();
    drop_external(cx, paths, to);
    shell.read_with(cx, |shell, cx| {
        let dropped = shell.composer.clone();
        assert_eq!(staged_names(&dropped, cx), ["notes.zip", "Image 2.png"]);
        assert_eq!(
            text(&dropped, cx),
            format!(
                "{} {} ",
                attachment_mention_link(1, Some("notes.zip")),
                attachment_mention_link(2, None)
            )
        );
        assert_eq!(
            dropped.read(cx).visible_failure().as_deref(),
            Some("folder is not a file. huge.bin is too large (24 MB max)."),
        );
    });
}
