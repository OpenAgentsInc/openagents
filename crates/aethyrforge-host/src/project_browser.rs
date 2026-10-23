//! Account project saves and explicit host-origin import/export, separate from engine execution.
use super::*;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Controller {
    pub save: Option<Prepared>,
    project: String,
    cursor: ProjectCursor,
    restored_visit: Option<StudioVisit>,
    opening: bool,
    open_serial: u64,
    epoch: u64,
    busy: bool,
    next_recovery: f64,
    last_recovery: String,
    next_access_check: f64,
    reopening_session: Option<String>,
}
pub(super) struct Prepared {
    project: String,
    version: u64,
    document: ProjectDocument,
    command: String,
    session: String,
    owner: Option<String>,
}
fn status(message: &str) {
    element("project-status").set_text_content(Some(message));
}
fn picker() -> web_sys::HtmlSelectElement {
    element("project-picker").unchecked_into()
}
fn key(s: &State) -> String {
    format!(
        "openagents.verse.recovery.v1:{}:{}:haunted-house:main",
        s.account.as_deref().unwrap_or("guest"),
        s.projects.project
    )
}
pub(super) fn clear(s: &mut State) {
    let reopening_session = s.snapshot()["session"].as_str().map(str::to_string);
    s.projects = Controller {
        epoch: s.projects.epoch.wrapping_add(1),
        reopening_session,
        ..Default::default()
    };
    picker().set_inner_html("<option value=\"\">Choose a project</option>");
    status("Choose a project for this account.");
}
pub(super) fn confirmation_scope(s: &State) -> Option<ConfirmationScope> {
    Some(ConfirmationScope {
        account: s.account.clone(),
        project: s.projects.project.clone(),
        epoch: s.projects.epoch,
        engine_session: s.snapshot()["session"].as_str()?.into(),
    })
}
pub(super) fn world_reloading(s: &mut State) {
    s.projects.reopening_session = s.snapshot()["session"].as_str().map(str::to_string);
    s.restore_complete = false;
}
async fn exported(weak: &Weak<RefCell<State>>) -> Result<ProjectDocument, JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    let scope = confirmation_scope(&rc.borrow()).ok_or(JsValue::NULL)?;
    let id = rc.borrow().request(&Action::ExportProject)?;
    for _ in 0..300 {
        pause(50).await;
        let s = rc.borrow();
        let snapshot = s.snapshot();
        if !scope.accepts(confirmation_scope(&s).as_ref()) {
            return Err(JsValue::from_str(
                "The world changed. Export the current world again.",
            ));
        }
        if let Some(r) = snapshot["receipts"]
            .as_array()
            .and_then(|rows| rows.iter().find(|r| r["request_id"] == id))
        {
            if r["outcome"]["state"] != "applied" {
                return Err(JsValue::from_str(
                    r["outcome"]["message"]
                        .as_str()
                        .unwrap_or("The editable copy could not be read."),
                ));
            }
            let mut d: ProjectDocument =
                serde_json::from_value(r["outcome"]["detail"]["document"].clone())
                    .map_err(|_| JsValue::from_str("The editable copy could not be read."))?;
            // Unfinished editor text belongs in the saved draft, even before Keep draft.
            d.source_draft = element("edit-source")
                .unchecked_into::<HtmlTextAreaElement>()
                .value();
            d.validate().map_err(|_| {
                JsValue::from_str("The editable copy is too large or contains unsupported values.")
            })?;
            return Ok(d);
        }
    }
    Err(JsValue::from_str(
        "The editable copy took too long. Your saved project has not changed.",
    ))
}
fn keep(rc: &Rc<RefCell<State>>, d: &ProjectDocument) -> Result<(), JsValue> {
    let mut s = rc.borrow_mut();
    s.save_visit();
    let value = json!({"owner":s.account,"project":s.projects.project,"version":s.projects.cursor.version,"document":d,"visit":s.visit});
    let storage = window().local_storage()?.ok_or_else(|| {
        JsValue::from_str("Device storage isn't available. Export an editable copy.")
    })?;
    storage
        .set_item(&key(&s), &value.to_string())
        .map_err(|_| {
            JsValue::from_str("Device storage is full or blocked. Export an editable copy.")
        })?;
    Ok(())
}
fn download(d: &ProjectDocument) -> Result<(), JsValue> {
    let text = serde_json::to_string_pretty(d).map_err(|_| JsValue::NULL)?;
    let a = window().document().unwrap().create_element("a")?;
    a.set_attribute(
        "href",
        &format!(
            "data:application/json;charset=utf-8,{}",
            js_sys::encode_uri_component(&text)
        ),
    )?;
    a.set_attribute("download", "openagents-world.json")?;
    let body = window().document().unwrap().body().unwrap();
    body.append_child(&a)?;
    a.unchecked_ref::<web_sys::HtmlElement>().click();
    a.remove();
    Ok(())
}
fn same(s: &State, account: &Option<String>, epoch: u64) -> bool {
    s.account == *account && s.projects.epoch == epoch
}
async fn list(weak: Weak<RefCell<State>>) -> Result<(), JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    let account = rc.borrow().account.clone();
    let epoch = rc.borrow().projects.epoch;
    let value = api("projects", None).await?;
    if !same(&rc.borrow(), &account, epoch) {
        return Ok(());
    }
    let selected = rc.borrow().projects.project.clone();
    picker().set_inner_html("<option value=\"\">Choose a project</option>");
    for p in value["data"].as_array().into_iter().flatten() {
        let Some(id) = p["id"].as_str() else { continue };
        let option = window().document().unwrap().create_element("option")?;
        option.set_attribute("value", id)?;
        option.set_text_content(Some(&format!(
            "{} ({})",
            p["name"].as_str().unwrap_or("Project"),
            p["role"].as_str().unwrap_or("viewer")
        )));
        picker().append_child(&option)?;
    }
    picker().set_value(&selected);
    status("Choose a project, then open its latest version or save this world.");
    Ok(())
}
async fn prepare_save(weak: Weak<RefCell<State>>) -> Result<(), JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    if rc.borrow().busy {
        return Err(JsValue::from_str(
            "Let the current change finish before saving.",
        ));
    }
    let d = exported(&weak).await?;
    if rc.borrow().busy {
        return Err(JsValue::from_str(
            "Let the current change finish before saving.",
        ));
    }
    let recovery = keep(&rc, &d);
    let mut s = rc.borrow_mut();
    let project = picker().value();
    if s.account.is_some() && project.is_empty() {
        status(
            "Choose or create a project first. Your editable copy is still available to export.",
        );
        return Ok(());
    }
    let command = s
        .projects
        .save
        .as_ref()
        .filter(|p| {
            p.project == project && p.version == s.projects.cursor.version && p.document == d
        })
        .map(|p| p.command.clone())
        .unwrap_or_else(random);
    s.projects.save = Some(Prepared {
        project,
        version: s.projects.cursor.version,
        document: d,
        command,
        session: s.snapshot()["session"].as_str().unwrap_or_default().into(),
        owner: s.account.clone(),
    });
    s.pending = None;
    s.persist();
    s.auth_card();
    if recovery.is_err() {
        status("Device storage could not keep a copy. Confirm a project save or export it.");
    }
    Ok(())
}
pub(super) async fn confirm_save(weak: Weak<RefCell<State>>) {
    let Some(rc) = weak.upgrade() else { return };
    let (account, epoch) = {
        let s = rc.borrow();
        (s.account.clone(), s.projects.epoch)
    };
    let result=async {
        let current=exported(&weak).await?;
        let (project, expected, body)={
            let mut s=rc.borrow_mut();
            if !same(&s,&account,epoch) || s.busy {return Ok(());}
            let p=s.projects.save.as_ref().ok_or(JsValue::NULL)?;
            if p.owner.as_ref().is_some_and(|o|Some(o)!=s.account.as_ref()) || p.document!=current || p.session!=s.snapshot()["session"].as_str().unwrap_or_default() {return Err(JsValue::from_str("The world or source draft changed. Review Save project again."));}
            if s.account.is_none() {s.auth_card();return Ok(());}
            if p.project.is_empty() {hide("edit-auth",true);status("You're signed in. Find or create a project, then choose Save project and confirm.");return Ok(());}
            let body=json!({"csrf":s.csrf,"command_id":p.command,"expected_version":p.version,"document":p.document,"summary":"Saved editable world"});
            let sent=(p.project.clone(),p.version,body);s.busy=true;sent
        };
        let saved=project_api(&weak,&format!("projects/{project}/haunted-house/main"),Some(body)).await;
        let cp={
            let mut s=rc.borrow_mut();
            if !same(&s,&account,epoch) {return Ok(());}
            s.busy=false;
            let value=saved?;
            if value["saved"]["document_sha256"].as_str()!=current.sha256().ok().as_deref() {return Err(JsValue::from_str("The save response did not match your content. Keep or export your device copy."));}
            let accepted=value["saved"]["version"].as_u64().ok_or(JsValue::NULL)?;
            let revision=s.snapshot()["status"]["project"]["revision"].as_u64().unwrap_or(MAX_SAFE_INTEGER);
            let source=element("edit-source").unchecked_into::<HtmlTextAreaElement>().value();
            let feedback=s.projects.cursor.saved(expected,accepted,current.source_revision,revision,&current.source_draft,&source).ok_or_else(||JsValue::from_str("The saved base changed. Open the latest project version before saving again."))?;
            s.projects.save=None;hide("edit-auth",true);
            if feedback==SaveFeedback::NewerEdits {edited(&s);}else{status(&format!("Saved to project · version {}. Publishing is a separate choice.",s.projects.cursor.version));}
            s.save_visit();
            json!({"csrf":s.csrf,"checkpoint":{"project":project,"namespace":"haunted-house","branch":"main","saved_version":s.projects.cursor.version,"visit":s.visit}})
        };
        let checkpoint_result=api(&format!("projects/{project}/haunted-house/main/checkpoint"),Some(cp)).await;
        let s=rc.borrow();
        if !same(&s,&account,epoch) {return Ok(());}
        if checkpoint_result.is_err() {
            let source=element("edit-source").unchecked_into::<HtmlTextAreaElement>().value();
            let newer=source!=current.source_draft || s.snapshot()["status"]["project"]["revision"].as_u64()!=Some(current.source_revision);
            status(if newer {"The earlier world version is saved. Newer edits and your place or tools still need saving; keep a device copy."}else{"The world is saved. Your place and tools could not save; keep a device copy before leaving."});
        }
        Ok::<_,JsValue>(())
    }.await;
    if let Err(e) = result {
        let s = rc.borrow();
        if same(&s, &account, epoch) {
            status(e.as_string().as_deref().unwrap_or("The project could not save. Keep your device copy or export it before trying again."));
        }
    }
}

async fn open(weak: Weak<RefCell<State>>, undo: bool) -> Result<(), JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    {
        let mut s = rc.borrow_mut();
        if s.busy {
            return Err(JsValue::from_str("Let the current save finish first."));
        }
        cancel_open(&mut s);
        s.projects.opening = true;
        s.projects.open_serial = s.projects.open_serial.wrapping_add(1);
    }
    let serial = rc.borrow().projects.open_serial;
    let (account, epoch, id, version, csrf) = {
        let s = rc.borrow();
        (
            s.account.clone(),
            s.projects.epoch,
            s.projects.project.clone(),
            s.projects.cursor.version,
            s.csrf.clone(),
        )
    };
    if id.is_empty() {
        return Err(JsValue::from_str("Choose a project first."));
    }
    let path = format!("projects/{id}/haunted-house/main");
    let v = if undo {
        project_api(
            &weak,
            &format!("{path}/undo"),
            Some(json!({"csrf":csrf,"command_id":random(),"expected_version":version})),
        )
        .await?
    } else {
        project_api(&weak, &path, None).await?
    };
    if !same(&rc.borrow(), &account, epoch) || rc.borrow().projects.open_serial != serial {
        return Ok(());
    }
    let head = if undo { &v["saved"] } else { &v["head"] };
    if head.is_null() {
        status("This project has no saved world yet. Choose Save project to keep this world.");
        return Ok(());
    }
    let document: ProjectDocument =
        serde_json::from_value(head["document"].clone()).map_err(|_| {
            JsValue::from_str(
                "This saved world could not be read. Export your device copy before trying again.",
            )
        })?;
    document
        .validate()
        .map_err(|_| JsValue::from_str("This saved world needs a compatible editor."))?;
    let version = head["version"].as_u64().ok_or(JsValue::NULL)?;
    rc.borrow_mut()
        .projects
        .cursor
        .stage_open(version, document.sha256().map_err(|_| JsValue::NULL)?);
    let kept = api(&format!("{path}/checkpoint"), None).await.ok();
    if !same(&rc.borrow(), &account, epoch) || rc.borrow().projects.open_serial != serial {
        return Ok(());
    }
    let visit = kept
        .and_then(|v| serde_json::from_value::<ProjectCheckpoint>(v["checkpoint"].clone()).ok())
        .filter(|c| {
            c.saved_version <= head["version"].as_u64().unwrap_or(0) && c.visit.owner == account
        })
        .map(|c| c.visit);
    let checkpoint = visit.as_ref().and_then(|v| v.checkpoint.clone());
    rc.borrow_mut().projects.restored_visit = visit;
    queue_change(
        &rc,
        Action::RestoreProject {
            document,
            checkpoint,
        },
    );
    status("Review opening this saved world. Your current world stays until you confirm.");
    Ok(())
}
async fn action(weak: Weak<RefCell<State>>, id: &str) -> Result<(), JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    match id {
        "project-list" => return list(weak).await,
        "project-create" => {
            let (account, epoch, csrf) = {
                let s = rc.borrow();
                (s.account.clone(), s.projects.epoch, s.csrf.clone())
            };
            let made = api(
                "projects",
                Some(json!({"csrf":csrf,"name":input("project-name").value()})),
            )
            .await?;
            if !same(&rc.borrow(), &account, epoch) {
                return Ok(());
            }
            // An uncertain create is never automatically retried: Find projects exposes it.
            list(weak).await?;
            if !same(&rc.borrow(), &account, epoch) {
                return Ok(());
            }
            let id = made["id"].as_str().ok_or(JsValue::NULL)?.to_string();
            picker().set_value(&id);
            {
                let mut s = rc.borrow_mut();
                s.projects.project = id;
                s.projects.epoch = s.projects.epoch.wrapping_add(1);
                s.projects.cursor = ProjectCursor::default();
                s.projects.restored_visit = None;
                s.projects.save = None;
            }
            status("Project created. Choose Save project to keep this world.");
        }
        "project-save" | "project-save-copy" => return prepare_save(weak).await,
        "project-open" => return open(weak, false).await,
        "project-undo" => return open(weak, true).await,
        "project-device" => {
            let d = exported(&weak).await?;
            keep(&rc, &d)?;
            status("Kept on this device only. Choose Save project to keep it in your account.");
        }
        "project-export" => {
            let d = exported(&weak).await?;
            download(&d)?;
            status("Editable copy exported. It includes unfinished source and original imports.");
        }
        "project-recover" => {
            cancel_open(&mut rc.borrow_mut());
            let s = rc.borrow();
            let raw = window()
                .local_storage()?
                .and_then(|storage| storage.get_item(&key(&s)).ok().flatten())
                .ok_or_else(|| {
                    JsValue::from_str("No device copy is available for this project and account.")
                })?;
            let v: Value = serde_json::from_str(&raw).map_err(|_| JsValue::NULL)?;
            if v["owner"] != json!(s.account) || v["project"] != s.projects.project {
                return Err(JsValue::from_str(
                    "Choose the account and project that kept this copy.",
                ));
            }
            let d: ProjectDocument =
                serde_json::from_value(v["document"].clone()).map_err(|_| JsValue::NULL)?;
            d.validate().map_err(|_| JsValue::NULL)?;
            drop(s);
            queue_change(
                &rc,
                Action::RestoreProject {
                    document: d,
                    checkpoint: None,
                },
            );
            status("Review opening the device copy. It has not been saved to your project.");
        }
        "project-import" => {
            cancel_open(&mut rc.borrow_mut());
            let files = input("project-import-files").files().ok_or(JsValue::NULL)?;
            if files.length() == 0 || files.length() > 8 {
                return Err(JsValue::from_str("Choose one to eight exported files."));
            }
            let account = rc.borrow().account.clone();
            let epoch = rc.borrow().projects.epoch;
            let mut originals = Vec::new();
            for index in 0..files.length() {
                let f = files.get(index).ok_or(JsValue::NULL)?;
                if f.size() > 24. * 1024. {
                    return Err(JsValue::from_str(
                        "Keep each imported source file below 24 KiB. Your original file has not changed.",
                    ));
                }
                let text = JsFuture::from(f.text())
                    .await?
                    .as_string()
                    .ok_or(JsValue::NULL)?;
                originals.push(OriginalImport {
                    name: f.name(),
                    sha256: project_hash(text.as_bytes()),
                    text,
                });
            }
            if !same(&rc.borrow(), &account, epoch) {
                return Ok(());
            }
            let (document, report) = if originals.len() == 1
                && serde_json::from_str::<ProjectDocument>(&originals[0].text).is_ok()
            {
                (
                    serde_json::from_str(&originals[0].text).unwrap(),
                    "Editable project imported for review.".into(),
                )
            } else {
                let baseline = exported(&weak).await?;
                let mapping: BTreeMap<String, String> = serde_json::from_str(
                    &element("project-import-mapping")
                        .unchecked_into::<HtmlTextAreaElement>()
                        .value(),
                )
                .map_err(|_| {
                    JsValue::from_str(
                        "Use a JSON object mapping old asset names to current asset IDs.",
                    )
                })?;
                let result=convert_legacy_files(originals,&mapping,baseline).map_err(|_|JsValue::from_str("These files exceed this editor's import bounds. Keep the original exports."))?;
                (result.document, result.unsupported.join("\n"))
            };
            document
                .validate()
                .map_err(|_| JsValue::from_str("This editable file is not supported."))?;
            if !same(&rc.borrow(), &account, epoch) {
                return Ok(());
            }
            element("project-import-report").set_text_content(Some(&report));
            queue_change(
                &rc,
                Action::RestoreProject {
                    document,
                    checkpoint: None,
                },
            );
        }
        _ => {}
    }
    Ok(())
}
pub(super) fn mount(
    rc: &Rc<RefCell<State>>,
    listeners: &mut Vec<(EventTarget, &'static str, Handler)>,
) -> Result<(), JsValue> {
    hide("project-tools", false);
    hide("project-save", false);
    for id in [
        "project-list",
        "project-create",
        "project-open",
        "project-save",
        "project-save-copy",
        "project-undo",
        "project-device",
        "project-recover",
        "project-export",
        "project-import",
    ] {
        let weak = Rc::downgrade(rc);
        listen(listeners, id, "click", move |_| {
            let weak = weak.clone();
            spawn_local(async move {
                let scope = weak.upgrade().map(|rc| {
                    let s = rc.borrow();
                    (s.account.clone(), s.projects.epoch, s.projects.open_serial)
                });
                if let Err(error) = action(weak.clone(), id).await {
                    if weak.upgrade().is_none_or(|rc| {
                        scope
                            .as_ref()
                            .is_none_or(|(account, epoch, _)| !same(&rc.borrow(), account, *epoch))
                    }) {
                        return;
                    }
                    status(error.as_string().as_deref().unwrap_or(
                        "This request could not finish. Your saved project has not changed.",
                    ));
                }
            });
        })?;
    }
    let weak = Rc::downgrade(rc);
    listen(listeners, "edit-source", "input", move |_| {
        if let Some(rc) = weak.upgrade() {
            edited(&rc.borrow());
        }
    })?;
    let weak = Rc::downgrade(rc);
    listen(listeners, "project-picker", "change", move |_| {
        if let Some(rc) = weak.upgrade() {
            let mut s = rc.borrow_mut();
            s.stop_input();
            s.pending = None;
            s.persist();
            s.projects.save = None;
            s.busy = false;
            s.projects.project = picker().value();
            s.projects.cursor.version = 0;
            s.projects.epoch = s.projects.epoch.wrapping_add(1);
            s.owned_world = None;
            let owner = s.account.clone();
            s.visit.discard_for(owner);
            s.selection = None;
            s.history.clear();
            note().set_value("");
            element("edit-source")
                .unchecked_into::<HtmlTextAreaElement>()
                .set_value("");
            element("edit-kiln-request")
                .unchecked_into::<HtmlTextAreaElement>()
                .set_value("");
            let _ = kiln_call("kilnVerseClear", None);
            s.demo.reload();
            s.projects.reopening_session = s.snapshot()["session"].as_str().map(str::to_string);
            s.restore_complete = false;
            hide("edit-auth", true);
            status("Project changed. Open its latest version or save a new world here.");
        }
    })?;
    Ok(())
}
pub(super) fn poll(rc: &Rc<RefCell<State>>) {
    let now = js_sys::Date::now();
    {
        let mut s = rc.borrow_mut();
        if s.snapshot()["phase"] == "open"
            && s.projects
                .reopening_session
                .as_ref()
                .is_some_and(|old| s.snapshot()["session"].as_str() != Some(old))
        {
            s.restore_complete = true;
            s.projects.reopening_session = None;
        }
    }
    let check = {
        let mut s = rc.borrow_mut();
        if s.account.is_some()
            && !s.projects.project.is_empty()
            && now >= s.projects.next_access_check
        {
            s.projects.next_access_check = now + 30000.;
            Some((
                s.account.clone(),
                s.projects.epoch,
                s.projects.project.clone(),
            ))
        } else {
            None
        }
    };
    if let Some((account, epoch, project)) = check {
        let weak = Rc::downgrade(rc);
        spawn_local(async move {
            if let Ok((code, value)) = api_response("projects", None).await {
                if let Some(rc) = weak.upgrade() {
                    if same(&rc.borrow(), &account, epoch)
                        && (matches!(code, 401 | 403)
                            || (code == 200
                                && value["data"]
                                    .as_array()
                                    .is_some_and(|rows| !rows.iter().any(|p| p["id"] == project))))
                    {
                        ended(&mut rc.borrow_mut());
                    }
                }
            }
        });
    }
    let s = rc.borrow();
    if s.projects.busy || now < s.projects.next_recovery || s.snapshot()["phase"] != "open" {
        return;
    }
    let stamp = format!(
        "{}:{}:{}",
        s.snapshot()["session"],
        s.snapshot()["status"]["project"]["revision"],
        element("edit-source")
            .unchecked_into::<HtmlTextAreaElement>()
            .value()
    );
    if stamp == s.projects.last_recovery {
        return;
    }
    let account = s.account.clone();
    let epoch = s.projects.epoch;
    drop(s);
    {
        let mut s = rc.borrow_mut();
        s.projects.busy = true;
        s.projects.next_recovery = now + 10000.;
    }
    let weak = Rc::downgrade(rc);
    spawn_local(async move {
        let result = exported(&weak).await;
        if let Some(rc) = weak.upgrade() {
            if !same(&rc.borrow(), &account, epoch) {
                return;
            }
            if let Ok(d) = result {
                if let Err(e) = keep(&rc, &d) {
                    status(
                        e.as_string()
                            .as_deref()
                            .unwrap_or("Device copy could not save. Export your world."),
                    );
                } else {
                    rc.borrow_mut().projects.last_recovery = stamp;
                }
            }
            rc.borrow_mut().projects.busy = false;
        }
    });
}

pub(super) fn restored(s: &mut State, action: &Value) {
    s.restore_complete = true;
    s.projects.reopening_session = None;
    let matching = serde_json::from_value::<ProjectDocument>(action["document"].clone())
        .ok()
        .and_then(|d| d.sha256().ok())
        .is_some_and(|h| s.projects.cursor.opened(&h));
    if !matching {
        s.projects.restored_visit = None;
    }
    s.projects.opening = false;
    if let Some(mut visit) = s.projects.restored_visit.take() {
        visit.owner = s.account.clone();
        visit.checkpoint =
            serde_json::from_value(s.snapshot()["status"]["checkpoint"].clone()).ok();
        s.visit = visit;
        note().set_value(&s.visit.note);
        element("edit-kiln-request")
            .unchecked_into::<HtmlTextAreaElement>()
            .set_value(&s.visit.request);
    }
    if let Some(draft) = action["document"]["source_draft"].as_str() {
        element("edit-source")
            .unchecked_into::<HtmlTextAreaElement>()
            .set_value(draft);
        s.visit.source = draft.into();
    }
    if let Some(id) = s.visit.selection.clone() {
        let _ = s.request(&Action::Select { target: id });
    }
    let _ = s.request(&Action::View {
        mode: if s.visit.mesh {
            ViewMode::Mesh
        } else {
            ViewMode::Shaded
        },
    });
    s.layout();
    s.save_visit();
    let requested = serde_json::from_value::<Checkpoint>(action["checkpoint"].clone()).ok();
    let actual =
        serde_json::from_value::<Checkpoint>(s.snapshot()["status"]["checkpoint"].clone()).ok();
    if requested.as_ref().is_some_and(|r| {
        actual.as_ref().is_none_or(|a| {
            r.mode != a.mode
                || r.eye
                    .iter()
                    .chain(&r.target)
                    .zip(a.eye.iter().chain(&a.target))
                    .any(|(r, a)| (r - a).abs() > 0.03)
        })
    }) {
        status(
            "Saved world opened. We used a clear starting place because the saved place was blocked or moved.",
        );
    } else {
        status("Saved world opened. Your editable source draft is ready.");
    }
}

pub(super) fn cancel_open(s: &mut State) {
    s.projects.open_serial = s.projects.open_serial.wrapping_add(1);
    s.projects.opening = false;
    s.projects.cursor.cancel_open();
    s.projects.restored_visit = None;
}
pub(super) fn edited(s: &State) {
    status(&if s.projects.cursor.version > 0 {
        format!(
            "Project version {} is saved. Newer edits aren't saved yet.",
            s.projects.cursor.version
        )
    } else {
        "Changes aren't saved to a project yet.".into()
    });
}

fn ended(s: &mut State) {
    s.stop_input();
    s.pending = None;
    s.persist();
    s.busy = false;
    s.owned_world = None;
    let owner = s.account.clone();
    s.visit.discard_for(owner);
    s.selection = None;
    s.history.clear();
    note().set_value("");
    element("edit-source")
        .unchecked_into::<HtmlTextAreaElement>()
        .set_value("");
    element("edit-kiln-request")
        .unchecked_into::<HtmlTextAreaElement>()
        .set_value("");
    let _ = kiln_call("kilnVerseClear", None);
    clear(s);
    s.demo.reload();
    s.restore_complete = false;
    hide("edit-auth", true);
    s.layout();
    status("Access to this project ended. Choose another project to continue.");
}
async fn project_api(
    weak: &Weak<RefCell<State>>,
    path: &str,
    body: Option<Value>,
) -> Result<Value, JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    let (account, epoch) = {
        let s = rc.borrow();
        (s.account.clone(), s.projects.epoch)
    };
    let (code, value) = api_response(path, body).await?;
    if !(200..300).contains(&code) {
        if matches!(code, 401 | 403 | 404) && same(&rc.borrow(), &account, epoch) {
            ended(&mut rc.borrow_mut());
        }
        return Err(JsValue::from_str(
            value["error"]["message"]
                .as_str()
                .unwrap_or("The project could not be checked. Keep a device copy or export it."),
        ));
    }
    Ok(value)
}
