//! Deliberate publication and rollback use native project authorization, never engine grants.
use super::*;
#[derive(Clone, PartialEq)]
struct ProjectScope {
    account: Option<String>,
    project: String,
    epoch: u64,
}
impl ProjectScope {
    fn of(s: &State) -> Self {
        Self {
            account: s.account.clone(),
            project: s.projects.project.clone(),
            epoch: s.projects.epoch,
        }
    }
    fn accepts(&self, s: &State) -> bool {
        self == &Self::of(s)
    }
}
#[derive(Clone)]
struct Prepared {
    scope: ProjectScope,
    engine: Option<ConfirmationScope>,
    context: Option<InputContext>,
    document: Option<ProjectDocument>,
    body: Value,
}
#[derive(Default)]
pub(super) struct Controller {
    baseline: Option<ProjectDocument>,
    baseline_session: String,
    capturing: bool,
    prepared: Option<Prepared>,
    latest: Option<Value>,
    selected: Option<String>,
}
fn message(text: &str) {
    element("publish-status").set_text_content(Some(text));
    say(text);
}
fn dialog(open: bool) {
    let d: web_sys::HtmlDialogElement = element("publish-review").unchecked_into();
    if open {
        hide("publish-review", false);
        let _ = d.show_modal();
    } else {
        d.close();
        hide("publish-review", true);
    }
}
fn clear_share() {
    hide("publish-share-card", true);
    element("publish-share-link").remove_attribute("href").ok();
    element("publish-share-link").set_text_content(None);
}
pub(super) fn context_changed(s: &mut State) {
    clear_share();
    s.projects.publication.prepared = None;
    s.projects.publication.latest = None;
    s.projects.publication.selected = None;
    for id in ["project-play", "project-share"] {
        let _ = element(id).set_attribute("disabled", "");
    }
    element("published-link").remove_attribute("href").ok();
    hide("published-link", true);
    element("publish-version").set_inner_html("");
    dialog(false);
}
fn scoped(s: &State, p: &Prepared) -> bool {
    p.scope.accepts(s)
        && p.engine
            .as_ref()
            .is_none_or(|engine| engine.accepts(confirmation_scope(s).as_ref()))
        && p.context
            .as_ref()
            .is_none_or(|context| s.input_context().as_ref() == Some(context))
}
fn render(s: &mut State, value: Value) {
    clear_share();
    let select = element("publish-version").unchecked_into::<web_sys::HtmlSelectElement>();
    select.set_inner_html("");
    for v in value["book"]["versions"].as_array().into_iter().flatten() {
        if v["revoked"] == true {
            continue;
        }
        if let Some(id) = v["release"].as_str() {
            let option = window()
                .document()
                .unwrap()
                .create_element("option")
                .unwrap();
            option.set_attribute("value", id).ok();
            option.set_text_content(Some(&format!(
                "Version {} · {}",
                v["saved_version"],
                if v["public"] == true {
                    "Public link"
                } else {
                    "Project members"
                }
            )));
            select.append_child(&option).ok();
        }
    }
    let channel = value["book"]["channel"].as_str();
    let valid = |id: &str| {
        value["book"]["versions"].as_array().is_some_and(|rows| {
            rows.iter()
                .any(|v| v["release"] == id && v["revoked"] != true)
        })
    };
    if let Some(selected) = s
        .projects
        .publication
        .selected
        .as_deref()
        .filter(|id| valid(id))
        .or(channel.filter(|id| valid(id)))
    {
        select.set_value(selected);
    }

    if let (Some(world), Some(release)) = (value["world"].as_str(), channel) {
        let link = format!("/new-lander/play/{world}/{release}");
        element("published-link").set_attribute("href", &link).ok();
        hide("published-link", false);
        element("published-link").set_text_content(Some("Open this playable version"));
        for id in ["project-play", "project-share"] {
            element(id).remove_attribute("disabled").ok();
        }
    } else {
        hide("published-link", true);
        for id in ["project-play", "project-share"] {
            element(id).set_attribute("disabled", "").ok();
        }
    }
    s.projects.publication.latest = Some(value);
}
async fn prepare(weak: Weak<RefCell<State>>, operation: &str) -> Result<(), JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    let (scope, selected) = {
        let mut s = rc.borrow_mut();
        if s.busy {
            return Err(JsValue::from_str("Let the current change finish first."));
        }
        if s.account.is_none() || s.projects.project.is_empty() {
            return Err(JsValue::from_str("Sign in and choose your project first."));
        }
        if operation == "publish" && s.projects.cursor.version == 0 {
            return Err(JsValue::from_str(
                "Save this world to a project before publishing.",
            ));
        }
        let scope = ProjectScope::of(&s);
        let selected = element("publish-version")
            .unchecked_into::<web_sys::HtmlSelectElement>()
            .value();
        s.busy = true;
        (scope, selected)
    };
    let result=async {
        let document=if operation=="publish" {
            let d=exported(&weak).await?;
            if materials::has_unsaved(&rc.borrow())||d.source_draft!=d.active_source{return Err(JsValue::from_str("Apply your face draft, compile your current effect, and save before publishing."));}
            Some(d)
        }else{None};
        let value=project_api(&weak,&format!("projects/{}/haunted-house/main/publications",scope.project),None).await?;
        let mut s=rc.borrow_mut();if !scope.accepts(&s){return Ok(());}
        if operation!="publish" && selected.is_empty(){return Err(JsValue::from_str("Choose a published version first."));}
        let managed=if operation!="publish" {Some(value["book"]["versions"].as_array().and_then(|rows|rows.iter().find(|v|v["release"]==selected&&v["revoked"]!=true)).cloned().ok_or_else(||JsValue::from_str("This version changed. Refresh published versions and choose again."))?)} else {None};
        let public=element("publish-visibility").unchecked_into::<web_sys::HtmlSelectElement>().value()=="public";
        let body=json!({"csrf":s.csrf,"command_id":random(),"expected_sequence":value["book"]["sequence"],"operation":operation,"saved_version":if document.is_some(){s.projects.cursor.version}else{0},"document_sha256":document.as_ref().map(ProjectDocument::sha256).transpose().map_err(|_|JsValue::NULL)?.unwrap_or_default(),"public":public,"confirm_runtime_distribution":true,"release":if document.is_some(){Value::Null}else{Value::String(selected)}});
        let (engine,context)=if document.is_some(){(Some(confirmation_scope(&s).ok_or(JsValue::NULL)?),Some(s.input_context().ok_or(JsValue::NULL)?))}else{(None,None)};
        s.projects.publication.prepared=Some(Prepared{scope:scope.clone(),engine,context,document,body});
        let version=managed.as_ref().map(|v|v["saved_version"].clone()).unwrap_or(json!(s.projects.cursor.version));
        let audience=if managed.as_ref().map(|v|v["public"]==true).unwrap_or(public){"Anyone with the link"}else{"Project members"};
        element("publish-review-target").set_text_content(Some(&format!("Version {version} · {audience}")));
        if let Some(v)=&managed {
            let link=format!("/new-lander/play/{}/{}",value["world"].as_str().ok_or(JsValue::NULL)?,v["release"].as_str().ok_or(JsValue::NULL)?);
            element("publish-review-link").set_attribute("href",&link)?;hide("publish-review-link",false);
        }else{hide("publish-review-link",true);}
        render(&mut s,value);input("publish-confirm-rights").set_checked(false);
        element("publish-review-title").set_text_content(Some(match operation{"rollback"=>"Return the shared link to this version?","revoke"=>"Turn off this playable link?",_=>"Publish this saved version?"}));
        element("publish-review-description").set_text_content(Some(match operation{"rollback"=>"New visits use the chosen version. Open visits stay on their opened version.","revoke"=>"New visits to this version will stop. Copies already opened or downloaded cannot be taken back.",_=>if public{"Anyone with the link can play this exact version. Later edits stay private until you publish again."}else{"Only current project members can open this exact version. Invitations are a separate choice."}}));
        element("publish-confirm-label").set_text_content(Some(match operation{"revoke"=>"Turn off new visits to this exact playable version.","rollback"=>"Send new visits of the shared link to this exact playable version.",_=>"Distribute this playable version under the applicable art and runtime terms. Keep editable imports private."}));
        dialog(true);say("Review this release. Say Confirm publish to confirm, or Cancel publish.");Ok::<_,JsValue>(())
    }.await;
    if scope.accepts(&rc.borrow()) {
        rc.borrow_mut().busy = false;
    }
    result
}
async fn confirm(weak: Weak<RefCell<State>>) {
    let Some(rc) = weak.upgrade() else { return };
    let Some(p) = rc.borrow().projects.publication.prepared.clone() else {
        return;
    };
    let mut owns_busy = false;
    let result = async {
        if !input("publish-confirm-rights").checked() {
            return Err(JsValue::from_str(
                "Confirm the playable distribution terms before publishing.",
            ));
        }
        {
            let mut s = rc.borrow_mut();
            if s.busy || !scoped(&s, &p) {
                return Err(JsValue::from_str(
                    "The world or account changed. Review this release again.",
                ));
            }
            s.busy = true;
            owns_busy = true;
        }
        if let Some(document) = &p.document {
            message("Checking this saved version…");
            let current = exported(&weak).await?;
            let s = rc.borrow();
            if !scoped(&s, &p) || &current != document || materials::has_unsaved(&s) {
                return Err(JsValue::from_str(
                    "Your world or draft changed. Save and review this release again.",
                ));
            }
        }
        message("Saving the playable release…");
        let value = project_api(
            &weak,
            &format!(
                "projects/{}/haunted-house/main/publications",
                p.scope.project
            ),
            Some(p.body.clone()),
        )
        .await?;
        let mut s = rc.borrow_mut();
        if !p.scope.accepts(&s) {
            return Ok(());
        }
        render(&mut s, value);
        s.projects.publication.prepared = None;
        dialog(false);
        message(match p.body["operation"].as_str() {
            Some("revoke") => "The playable link is turned off for new visits.",
            Some("rollback") => {
                "The shared link now opens the chosen version. Open visits keep their version."
            }
            _ => "Published. Choose Play or Share to open this exact version.",
        });
        Ok::<_, JsValue>(())
    }
    .await;
    if p.scope.accepts(&rc.borrow()) {
        if owns_busy {
            rc.borrow_mut().busy = false;
        }
        if let Err(e) = result {
            message(e.as_string().as_deref().unwrap_or(
                "The release could not finish. Your previous playable version is still available.",
            ));
        }
    }
}
pub(super) fn poll(rc: &Rc<RefCell<State>>) {
    let candidate = {
        let mut s = rc.borrow_mut();
        let snapshot = s.snapshot();
        let session = snapshot["session"].as_str().unwrap_or_default().to_string();
        if s.projects.publication.baseline_session != session {
            s.projects.publication.baseline = None;
            s.projects.publication.capturing = false;
            s.projects.publication.baseline_session = session.clone();
        }
        if snapshot["phase"] != "open"
            || snapshot["status"]["project"]["revision"] != 0
            || s.projects.publication.baseline.is_some()
            || s.projects.publication.capturing
        {
            return;
        }
        s.projects.publication.capturing = true;
        session
    };
    let weak = Rc::downgrade(rc);
    spawn_local(async move {
        let result = exported(&weak).await;
        let Some(rc) = weak.upgrade() else { return };
        let mut s = rc.borrow_mut();
        if s.projects.publication.baseline_session != candidate {
            return;
        }
        s.projects.publication.capturing = false;
        if let Ok(mut document) = result {
            if document.source_revision == 0 {
                document.source_draft = document.active_source.clone();
                s.projects.publication.baseline = Some(document);
                element("project-revert").remove_attribute("disabled").ok();
            }
        }
    });
}
pub(super) fn mount(
    rc: &Rc<RefCell<State>>,
    listeners: &mut Vec<(EventTarget, &'static str, Handler)>,
) -> Result<(), JsValue> {
    let enabled = element("publish-enabled")
        .get_attribute("content")
        .as_deref()
        == Some("true");
    hide("publish-tools", !enabled);
    for id in ["project-publish", "project-play", "project-share"] {
        hide(id, !enabled);
    }
    hide("project-revert", false);
    for (id, operation) in [
        ("project-publish", "publish"),
        ("publish-rollback", "rollback"),
        ("publish-revoke", "revoke"),
    ] {
        let weak = Rc::downgrade(rc);
        listen(listeners, id, "click", move |_| {
            let weak = weak.clone();
            spawn_local(async move {
                if let Err(e) = prepare(weak, operation).await {
                    message(
                        e.as_string()
                            .as_deref()
                            .unwrap_or("The release could not open."),
                    );
                }
            });
        })?;
    }
    let weak = Rc::downgrade(rc);
    listen(listeners, "publish-refresh", "click", move |_| {
        let weak = weak.clone();
        spawn_local(async move {
            let Some(rc) = weak.upgrade() else { return };
            let scope = {
                let s = rc.borrow();
                if s.account.is_none() || s.projects.project.is_empty() {
                    return;
                }
                ProjectScope::of(&s)
            };
            let result = project_api(
                &weak,
                &format!("projects/{}/haunted-house/main/publications", scope.project),
                None,
            )
            .await;
            if !scope.accepts(&rc.borrow()) {
                return;
            }
            match result {
                Ok(v) => {
                    render(&mut rc.borrow_mut(), v);
                    message("Published versions refreshed.");
                }
                Err(_) => message(
                    "Published versions could not open. Choose a saved project and try again.",
                ),
            }
        });
    })?;
    let weak = Rc::downgrade(rc);
    listen(listeners, "publish-confirm", "click", move |_| {
        spawn_local(confirm(weak.clone()));
    })?;
    let weak = Rc::downgrade(rc);
    listen(listeners, "publish-cancel", "click", move |_| {
        if let Some(rc) = weak.upgrade() {
            if rc.borrow().busy {
                return;
            }
            rc.borrow_mut().projects.publication.prepared = None;
        }
        dialog(false);
        message("Release canceled. Your private world is still here.");
    })?;
    let weak = Rc::downgrade(rc);
    listen(listeners, "project-revert", "click", move |_| {
        let Some(rc) = weak.upgrade() else { return };
        let s = rc.borrow();
        if s.busy {
            return;
        }
        let baseline = s.projects.publication.baseline.clone();
        let checkpoint = s.snapshot()["status"]["checkpoint"].clone();
        drop(s);
        if let Some(document) = baseline {
            message(
                "Review returning to Starting house. Saved and published versions stay available.",
            );
            queue_change(
                &rc,
                Action::RestoreProject {
                    document,
                    checkpoint: serde_json::from_value(checkpoint).ok(),
                },
            );
        }
    })?;
    for id in ["project-play", "project-share"] {
        let weak = Rc::downgrade(rc);
        listen(listeners, id, "click", move |_| {
            let Some(rc) = weak.upgrade() else { return };
            let Some(path) = element("published-link").get_attribute("href") else {
                return;
            };
            let Some(ids) = path.strip_prefix("/new-lander/play/") else {
                return;
            };
            let Some((world, release)) = ids.split_once('/') else {
                return;
            };
            let (scope, csrf) = {
                let s = rc.borrow();
                (ProjectScope::of(&s), s.csrf.clone())
            };
            let body = json!({"csrf":csrf,"operation":if id=="project-play" {"play"}else{"share"},"world":world,"release":release});
            let weak = weak.clone();
            spawn_local(async move {
                let progress = api_response("tutorial", Some(body)).await;
                let Some(rc) = weak.upgrade() else { return };
                if !scope.accepts(&rc.borrow())
                    || element("published-link").get_attribute("href").as_deref() != Some(&path)
                {
                    return;
                }
                if id == "project-play" {
                    let _ = window().location().set_href(&path);
                } else {
                    let origin = window().location().origin().unwrap_or_default();
                    element("publish-share-link")
                        .set_text_content(Some(&format!("{origin}{path}")));
                    element("publish-share-link")
                        .set_attribute("href", &path)
                        .ok();
                    hide("publish-share-card", false);
                    say(
                        if progress
                            .as_ref()
                            .is_ok_and(|(status, _)| (200..300).contains(status))
                        {
                            "Your playable link is ready. Return to the planets to create your own planet."
                        } else {
                            "Your playable link is ready. Tutorial progress could not be saved; try Share again before returning to the planets."
                        },
                    );
                }
            });
        })?;
    }
    let weak = Rc::downgrade(rc);
    listen(listeners, "publish-version", "change", move |_| {
        if let Some(rc) = weak.upgrade() {
            rc.borrow_mut().projects.publication.selected = Some(
                element("publish-version")
                    .unchecked_into::<web_sys::HtmlSelectElement>()
                    .value(),
            );
        }
    })?;
    listen(listeners, "publish-share-close", "click", move |_| {
        hide("publish-share-card", true)
    })?;
    Ok(())
}
