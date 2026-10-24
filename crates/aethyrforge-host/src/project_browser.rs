//! Account project saves and explicit host-origin import/export, separate from engine execution.
use super::*;
use std::collections::BTreeMap;
#[path = "project_material_browser.rs"]
mod files;
#[path = "history_browser.rs"]
mod history;
#[path = "publish_browser.rs"]
mod publishing;

#[derive(Default)]
pub(super) struct Controller {
    history: history::View,
    publication: publishing::Controller,
    images: BTreeMap<String, MaterialImage>,
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
    planet_recipe: Option<PlanetRecipe>,
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
        "openagents.verse.recovery.v1:{}:{}:{}:main",
        s.account.as_deref().unwrap_or("guest"),
        s.projects.project,
        world_namespace().key()
    )
}
pub(super) fn bind_personal(s: &mut State) {
    if world_namespace() != WorldNamespace::PersonalPlanet {
        return;
    }
    let meta = |id| {
        window()
            .document()
            .and_then(|d| d.get_element_by_id(id))
            .and_then(|e| e.get_attribute("content"))
    };
    if s.account.is_none() || s.account != meta("personal-owner") {
        return;
    }
    let Some(project) = meta("personal-project") else {
        return;
    };
    if s.projects.project == project {
        return;
    }
    change_project(s, project.clone());
    picker().set_inner_html("");
    if let Ok(option) = window().document().unwrap().create_element("option") {
        let _ = option.set_attribute("value", &project);
        option.set_text_content(Some("Your planet"));
        let _ = picker().append_child(&option);
        picker().set_value(&project);
    }
    status("Generate your courtyard, or open its latest saved version.");
}
pub(super) fn clear(s: &mut State) {
    publishing::context_changed(s);
    history::clear();
    let reopening_session = s.snapshot()["session"].as_str().map(str::to_string);
    s.projects = Controller {
        epoch: s.projects.epoch.wrapping_add(1),
        reopening_session,
        ..Default::default()
    };
    picker().set_inner_html("<option value=\"\">Choose a project</option>");
    status("Choose a project for this account.");
}
fn change_project(s: &mut State, project: String) {
    publishing::context_changed(s);
    // Recovery callbacks own their starting epoch. The new context starts idle,
    // even when its predecessor still has an export in flight.
    s.projects.project = project;
    s.projects.epoch = s.projects.epoch.wrapping_add(1);
    s.projects.cursor = ProjectCursor::default();
    s.projects.save = None;
    s.projects.restored_visit = None;
    s.projects.opening = false;
    s.projects.open_serial = s.projects.open_serial.wrapping_add(1);
    s.projects.busy = false;
    s.projects.next_recovery = 0.;
    s.projects.last_recovery.clear();
    s.projects.next_access_check = 0.;
    s.projects.reopening_session = None;
    s.projects.history = history::View::default();
    history::clear();
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
    materials::clear(s);
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
    let archive = files::cached(&s, d)?;
    let value = json!({"owner":s.account,"project":s.projects.project,"version":s.projects.cursor.version,"document":d,"materials":archive.materials,"visit":s.visit});
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
    if materials::has_unsaved(&rc.borrow()) {
        return Err(JsValue::from_str(
            "Apply your face draft, or export and discard it, before saving this world.",
        ));
    }
    if rc.borrow().busy {
        return Err(JsValue::from_str(
            "Let the current change finish before saving.",
        ));
    }
    let d = exported(&weak).await?;
    if materials::has_unsaved(&rc.borrow()) {
        return Err(JsValue::from_str(
            "Apply your face draft, or export and discard it, before saving this world.",
        ));
    }
    if rc.borrow().busy {
        return Err(JsValue::from_str(
            "Let the current change finish before saving.",
        ));
    }
    let _archive = files::collect(&weak, &d).await?;
    if materials::has_unsaved(&rc.borrow()) {
        return Err(JsValue::from_str(
            "Apply or export your face draft before saving this world.",
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
        if materials::has_unsaved(&rc.borrow()) {return Err(JsValue::from_str("Your face draft changed. Apply it, or export and discard it, then review Save project again."));}
        let current=exported(&weak).await?;
        let archive=files::collect(&weak,&current).await?;
        let (project, expected, body)={
            let mut s=rc.borrow_mut();
            if !same(&s,&account,epoch) || s.busy {return Ok(());}
            if materials::has_unsaved(&s) {return Err(JsValue::from_str("Your face draft changed. Apply it, or export and discard it, then review Save project again."));}
            let p=s.projects.save.as_ref().ok_or(JsValue::NULL)?;
            if p.owner.as_ref().is_some_and(|o|Some(o)!=s.account.as_ref()) || p.document!=current || current.source_draft!=element("edit-source").unchecked_into::<HtmlTextAreaElement>().value() || s.snapshot()["status"]["project"]["revision"].as_u64()!=Some(current.source_revision) || p.session!=s.snapshot()["session"].as_str().unwrap_or_default() {return Err(JsValue::from_str("The world or source draft changed. Review Save project again."));}
            if s.account.is_none() {s.auth_card();return Ok(());}
            if p.project.is_empty() {hide("edit-auth",true);status("You're signed in. Find or create a project, then choose Save project and confirm.");return Ok(());}
            let body=json!({"csrf":s.csrf,"command_id":p.command,"expected_version":p.version,"document":p.document,"summary":"Saved editable world"});
            let sent=(p.project.clone(),p.version,body);s.busy=true;sent
        };
        let saved=async {
            files::upload(&weak,&project,&archive).await?;
            {
                let s=rc.borrow();
                if !same(&s,&account,epoch) || materials::has_unsaved(&s) || current.source_draft!=element("edit-source").unchecked_into::<HtmlTextAreaElement>().value() || s.snapshot()["status"]["project"]["revision"].as_u64()!=Some(current.source_revision) {
                    return Err(JsValue::from_str("The world or a draft changed. Review Save project again."));
                }
            }
            project_api(&weak,&project_path(&project),Some(body)).await
        }.await;
        let cp={
            let mut s=rc.borrow_mut();
            if !same(&s,&account,epoch) {return Ok(());}
            s.busy=false;
            let value=saved?;
            if value["saved"]["document_sha256"].as_str()!=current.sha256().ok().as_deref() {return Err(JsValue::from_str("The save response did not match your content. Keep or export your device copy."));}
            let accepted=value["saved"]["version"].as_u64().ok_or(JsValue::NULL)?;
            let revision=s.snapshot()["status"]["project"]["revision"].as_u64().unwrap_or(MAX_SAFE_INTEGER);
            let source=element("edit-source").unchecked_into::<HtmlTextAreaElement>().value();
            let newer_material=materials::has_unsaved(&s);
            let feedback=s.projects.cursor.saved(expected,accepted,current.source_revision,revision,&current.source_draft,&source,newer_material).ok_or_else(||JsValue::from_str("The saved base changed. Open the latest project version before saving again."))?;
            s.projects.save=None;hide("edit-auth",true);
            if feedback==SaveFeedback::NewerEdits {edited(&s);}else{status(&format!("Saved to project · version {}. Publishing is a separate choice.",s.projects.cursor.version));}
            s.save_visit();
            json!({"csrf":s.csrf,"checkpoint":{"project":project,"namespace":world_namespace(),"branch":"main","saved_version":s.projects.cursor.version,"visit":s.visit}})
        };
        let checkpoint_result=api(&format!("{}/checkpoint",project_path(&project)),Some(cp)).await;
        let s=rc.borrow();
        if !same(&s,&account,epoch) {return Ok(());}
        if checkpoint_result.is_err() {
            let source=element("edit-source").unchecked_into::<HtmlTextAreaElement>().value();
            let newer=source!=current.source_draft || s.snapshot()["status"]["project"]["revision"].as_u64()!=Some(current.source_revision) || materials::has_unsaved(&s);
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
    let path = project_path(&id);
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
    // Choosing a project replaces the engine session. Keep the user's Open
    // request until that session can receive the restore, or the context changes.
    let mut ready = false;
    for _ in 0..2400 {
        {
            let s = rc.borrow();
            if !same(&s, &account, epoch) || s.projects.open_serial != serial {
                return Ok(());
            }
            let snapshot = s.snapshot();
            if snapshot["phase"] == "open"
                && serde_json::from_value::<Checkpoint>(snapshot["status"]["checkpoint"].clone())
                    .is_ok()
            {
                ready = true;
                break;
            }
            if matches!(snapshot["phase"].as_str(), Some("failed" | "closed")) {
                return Err(JsValue::from_str(
                    "Reload the world, then open your saved version again.",
                ));
            }
        }
        status("Opening the world before reviewing your saved version…");
        pause(50).await;
    }
    if !ready {
        return Err(JsValue::from_str(
            "The world is taking longer to open. Reload it and try again.",
        ));
    }
    let action = files::restore(&weak, &id, document, checkpoint).await;
    if !same(&rc.borrow(), &account, epoch) || rc.borrow().projects.open_serial != serial {
        return Ok(());
    }
    let action = match action {
        Ok(action) => action,
        Err(error) => {
            cancel_open(&mut rc.borrow_mut());
            return Err(error);
        }
    };
    rc.borrow_mut().projects.restored_visit = visit;
    queue_change(&rc, action);
    status("Review opening this saved world. Your current world stays until you confirm.");
    Ok(())
}
async fn action(weak: Weak<RefCell<State>>, id: &str) -> Result<(), JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    if world_namespace() == WorldNamespace::PersonalPlanet
        && matches!(id, "project-list" | "project-create")
    {
        return Err(JsValue::from_str(
            "This world uses your awarded planet project.",
        ));
    }
    match id {
        "project-list" => return list(weak).await,
        "project-create" => {
            let (scope, csrf, name) = {
                let mut s = rc.borrow_mut();
                if s.busy {
                    return Err(JsValue::from_str(
                        "Let the current change or save finish before creating a project.",
                    ));
                }
                let scope = confirmation_scope(&s).ok_or_else(|| {
                    JsValue::from_str("Wait for the world to open before creating a project.")
                })?;
                s.busy = true;
                (scope, s.csrf.clone(), input("project-name").value())
            };
            status("Creating project…");
            let result = async {
                let made = api("projects", Some(json!({"csrf":csrf,"name":name}))).await?;
                require_scope(&weak, &scope)?;
                // An uncertain create is never automatically retried: Find projects exposes it.
                list(weak.clone()).await?;
                require_scope(&weak, &scope)?;
                made["id"].as_str().map(str::to_string).ok_or(JsValue::NULL)
            }
            .await;
            let mut s = rc.borrow_mut();
            if !same(&s, &scope.account, scope.epoch) {
                return Ok(());
            }
            s.busy = false;
            if !scope.accepts(confirmation_scope(&s).as_ref()) {
                return Ok(());
            }
            let id = result?;
            picker().set_value(&id);
            change_project(&mut s, id);
            s.pending = None;
            s.persist();
            hide("edit-auth", true);
            status("Project created. Choose Save project to keep this world.");
        }
        "project-save" | "project-save-copy" => return prepare_save(weak).await,
        "project-open" => return open(weak, false).await,
        "project-undo" => return open(weak, true).await,
        "project-device" => {
            let d = exported(&weak).await?;
            files::collect(&weak, &d).await?;
            keep(&rc, &d)?;
            status("Kept on this device only. Choose Save project to keep it in your account.");
        }
        "project-export" => {
            let d = exported(&weak).await?;
            let archive = files::collect(&weak, &d).await?;
            files::download(&archive)?;
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
            let checkpoint = serde_json::from_value::<StudioVisit>(v["visit"].clone())
                .ok()
                .and_then(|visit| visit.owned_checkpoint(s.account.as_deref()));
            let action = if let Some(images) = v.get("materials") {
                let archive = MaterialProjectArchive {
                    schema: "openagents.verse.project-export.v2".into(),
                    document: d,
                    materials: serde_json::from_value(images.clone()).map_err(|_| {
                        JsValue::from_str(
                            "The device copy is missing its face images. Keep your exported files.",
                        )
                    })?,
                };
                archive.restore(checkpoint).map_err(|_| {
                    JsValue::from_str("The device copy needs its matching face images.")
                })?
            } else {
                if !project_materials(&d).map_err(|_| JsValue::NULL)?.is_empty() {
                    return Err(JsValue::from_str(
                        "This older device copy has no face images. Open the account save or your complete exported world.",
                    ));
                }
                Action::RestoreProject {
                    document: d,
                    checkpoint,
                }
            };
            drop(s);
            if let Action::RestoreMaterialProject { materials, .. } = &action {
                for bundle in materials {
                    files::remember(&mut rc.borrow_mut(), bundle);
                }
            }
            queue_change(&rc, action);
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
            let serial = rc.borrow().projects.open_serial;
            if files.length() == 1 {
                let file = files.get(0).ok_or(JsValue::NULL)?;
                if file.size() > 1024. * 1024. {
                    return Err(JsValue::from_str(
                        "Choose an editable world export up to 1 MiB.",
                    ));
                }
                let text = JsFuture::from(file.text())
                    .await?
                    .as_string()
                    .ok_or(JsValue::NULL)?;
                if !same(&rc.borrow(), &account, epoch)
                    || rc.borrow().projects.open_serial != serial
                {
                    return Ok(());
                }
                let action = if let Ok(archive) =
                    serde_json::from_str::<MaterialProjectArchive>(&text)
                {
                    let action=archive.restore(None).map_err(|_|JsValue::from_str("This exported world has an unsupported or missing face image. Keep the original file."))?;
                    for bundle in &archive.materials {
                        files::remember(&mut rc.borrow_mut(), bundle);
                    }
                    Some(action)
                } else if let Ok(document) = serde_json::from_str::<ProjectDocument>(&text) {
                    document.validate().map_err(|_| {
                        JsValue::from_str("This world export is too large or unsupported.")
                    })?;
                    if !project_materials(&document)
                        .map_err(|_| JsValue::NULL)?
                        .is_empty()
                    {
                        return Err(JsValue::from_str(
                            "Open the complete world export that includes its face images.",
                        ));
                    }
                    Some(Action::RestoreProject {
                        document,
                        checkpoint: None,
                    })
                } else {
                    None
                };
                if let Some(action) = action {
                    element("project-import-report").set_text_content(Some(
                        "Editable world and source images opened for review.",
                    ));
                    queue_change(&rc, action);
                    return Ok(());
                }
            }
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
            if !same(&rc.borrow(), &account, epoch) || rc.borrow().projects.open_serial != serial {
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
            if !same(&rc.borrow(), &account, epoch) || rc.borrow().projects.open_serial != serial {
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
pub(super) fn preparing_planet(s: &State) {
    if world_namespace() == WorldNamespace::PersonalPlanet
        && s.pending.as_ref().is_some_and(|p| {
            matches!(
                p.pending.action,
                Action::RestoreProject { .. } | Action::RestoreMaterialProject { .. }
            )
        })
    {
        planet_note("Preparing your courtyard… Cancel to keep the current world.");
    }
}
pub(super) fn cancel_planet(s: &mut State) -> bool {
    if world_namespace() != WorldNamespace::PersonalPlanet
        || !s.busy
        || !s.pending.as_ref().is_some_and(|p| {
            matches!(
                p.pending.action,
                Action::RestoreProject { .. } | Action::RestoreMaterialProject { .. }
            )
        })
    {
        return false;
    }
    s.projects.epoch = s.projects.epoch.wrapping_add(1);
    s.projects.busy = false;
    s.projects.next_recovery = 0.;
    s.projects.last_recovery.clear();
    s.projects.open_serial = s.projects.open_serial.wrapping_add(1);
    s.projects.opening = false;
    s.projects.restored_visit = None;
    s.projects.save = None;
    s.projects.next_access_check = 0.;
    s.projects.history = history::View::default();
    history::clear();
    publishing::context_changed(s);
    s.pending = None;
    s.busy = false;
    s.persist();
    let _ = s.request(&Action::RevokeAuthorization);
    planet_note(
        "Cancellation requested. Your current courtyard is still here; review it before trying again.",
    );
    true
}
fn planet_note(message: &str) {
    if let Some(e) = window()
        .document()
        .and_then(|d| d.get_element_by_id("planet-status"))
    {
        e.set_text_content(Some(message));
    }
    say(message);
}
fn planet_settings(recipe: &PlanetRecipe) {
    element("planet-style")
        .unchecked_into::<web_sys::HtmlSelectElement>()
        .set_value(match recipe.style {
            PlanetStyle::SoftPastels => "soft-pastels",
            PlanetStyle::BoldFantasy => "bold-fantasy",
        });
    input("planet-hour").set_value(&recipe.initial_hour.to_string());
    input("planet-day").set_value(&recipe.day_minutes.to_string());
    input("planet-seed").set_value(&recipe.seed.to_string());
    input("planet-cycle").set_checked(recipe.cycle_enabled);
}
async fn prepare_planet(
    weak: Weak<RefCell<State>>,
    regenerate: bool,
    change: Option<PlanetChange>,
) -> Result<(), JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    let (scope, recipe) = {
        let mut s = rc.borrow_mut();
        if s.busy || s.account.is_none() || s.projects.project.is_empty() {
            return Err(JsValue::from_str(
                "Sign in and let the current change finish first.",
            ));
        }
        if materials::has_unsaved(&s) {
            return Err(JsValue::from_str(
                "Apply or export your face draft before changing the courtyard.",
            ));
        }
        let scope = confirmation_scope(&s)
            .ok_or_else(|| JsValue::from_str("Wait for the courtyard to open."))?;
        let recipe = if change.is_some() {
            PlanetRecipe::new(0, js_sys::Date::now() as u64)
        } else {
            let seed = input("planet-seed").value_as_number();
            let minutes = input("planet-day").value_as_number();
            if !seed.is_finite()
                || seed.fract() != 0.
                || !(0.0..=u32::MAX as f64).contains(&seed)
                || !minutes.is_finite()
                || minutes.fract() != 0.
                || !(2.0..=60.0).contains(&minutes)
            {
                return Err(JsValue::from_str(
                    "Choose a whole layout seed and a day length from 2 to 60 minutes.",
                ));
            }
            let mut recipe = PlanetRecipe::new(seed as u32, js_sys::Date::now() as u64);
            recipe.style = match element("planet-style")
                .unchecked_into::<web_sys::HtmlSelectElement>()
                .value()
                .as_str()
            {
                "soft-pastels" => PlanetStyle::SoftPastels,
                "bold-fantasy" => PlanetStyle::BoldFantasy,
                _ => return Err(JsValue::from_str("Choose Soft pastels or Bold fantasy.")),
            };
            recipe.initial_hour = input("planet-hour").value_as_number();
            recipe.cycle_enabled = input("planet-cycle").checked();
            recipe.day_minutes = minutes as u16;
            recipe
                .validate()
                .map_err(|_| JsValue::from_str("Choose a starting hour from 0 to under 24."))?;
            recipe
        };
        s.busy = true;
        s.projects.save = None;
        (scope, recipe)
    };
    let result = async {
        let mut accepted_recipe = recipe.clone();
        let mut document = if regenerate {
            let template: WorldTemplate = serde_json::from_str(
                &element("personal-template")
                    .text_content()
                    .unwrap_or_default(),
            )
            .map_err(|_| JsValue::from_str("Reload to open this courtyard's settings."))?;
            recipe
                .document(template)
                .map_err(|_| JsValue::from_str("This courtyard could not be prepared."))?
        } else {
            let mut d = exported(&weak).await?;
            let prior = PlanetRecipe::from_document(&d).map_err(|_| {
                JsValue::from_str("Generate a courtyard or open your saved world first.")
            })?;
            let change = change.unwrap_or(PlanetChange::Settings {
                style: recipe.style,
                hour: recipe.initial_hour,
                cycle: recipe.cycle_enabled,
                day_minutes: recipe.day_minutes,
            });
            accepted_recipe = prior
                .changed(change, js_sys::Date::now() as u64)
                .map_err(|_| JsValue::from_str("Choose a supported style, hour, or day length."))?;
            if d.source_draft != d.active_source {
                return Err(JsValue::from_str(
                    "Keep your unfinished effect before choosing a new world style.",
                ));
            }
            if accepted_recipe.style != prior.style || matches!(change, PlanetChange::Style(_)) {
                d.active_source = accepted_recipe.style.effect();
                d.source_draft = d.active_source.clone();
            }
            d.environment = accepted_recipe.lighting_at(js_sys::Date::now() as u64);
            d.originals.retain(|s| s.name != PLANET_SOURCE);
            d.originals
                .push(accepted_recipe.source().map_err(|_| JsValue::NULL)?);
            d
        };
        if !scope.accepts(confirmation_scope(&rc.borrow()).as_ref()) {
            return Ok(None);
        }
        if materials::has_unsaved(&rc.borrow()) {
            return Err(JsValue::from_str(
                "Your face draft changed. Keep or export it before trying again.",
            ));
        }
        document.source_revision = rc.borrow().snapshot()["status"]["project"]["revision"]
            .as_u64()
            .unwrap_or(0);
        let action = if regenerate {
            Action::RestoreProject {
                document,
                checkpoint: None,
            }
        } else {
            let checkpoint =
                serde_json::from_value(rc.borrow().snapshot()["status"]["checkpoint"].clone()).ok();
            files::restore(&weak, &scope.project, document, checkpoint).await?
        };
        if !scope.accepts(confirmation_scope(&rc.borrow()).as_ref()) {
            return Ok(None);
        }
        Ok::<_, JsValue>(Some((action, accepted_recipe)))
    }
    .await;
    if !scope.accepts(confirmation_scope(&rc.borrow()).as_ref()) {
        return Ok(());
    }
    rc.borrow_mut().busy = false;
    if let Some((action, recipe)) = result? {
        let message = if regenerate {
            format!(
                "Review replacing this courtyard with 24 kit objects in {}. Confirm change to generate; Cancel keeps the current world.",
                recipe.style.label()
            )
        } else {
            format!(
                "Review {} · Starting time {} · Day cycle {}. Confirm change to apply.",
                recipe.style.label(),
                recipe.time_label(),
                if recipe.cycle_enabled {
                    "running"
                } else {
                    "paused"
                }
            )
        };
        queue_change(&rc, action);
        planet_note(&message);
    }
    Ok(())
}
pub(super) async fn history_applied(weak: &Weak<RefCell<State>>, scope: &ConfirmationScope) {
    if world_namespace() != WorldNamespace::PersonalPlanet {
        return;
    }
    for _ in 0..6 {
        if require_scope(weak, scope).is_err() {
            return;
        }
        match exported(weak).await {
            Ok(document) => {
                if require_scope(weak, scope).is_err() {
                    return;
                }
                let Ok(recipe) = PlanetRecipe::from_document(&document) else {
                    return;
                };
                let Some(rc) = weak.upgrade() else {
                    return;
                };
                let mut s = rc.borrow_mut();
                let source = element("edit-source").unchecked_into::<HtmlTextAreaElement>();
                if s.projects
                    .planet_recipe
                    .as_ref()
                    .is_some_and(|old| source.value() == old.style.effect())
                {
                    if let Some(draft) = s.snapshot()["status"]["authoring"]["draft"].as_str() {
                        source.set_value(draft);
                        s.visit.source = draft.into();
                    }
                }
                planet_settings(&recipe);
                s.projects.planet_recipe = Some(recipe);
                planet_note(
                    "World settings restored. Unconfirmed style and time choices were reset.",
                );
                return;
            }
            Err(_) => pause(100).await,
        }
    }
    if require_scope(weak, scope).is_ok() {
        planet_note(
            "The world changed, but its controls could not refresh. Open the latest saved world before reviewing settings.",
        );
    }
}
pub(super) fn personal_request(rc: &Rc<RefCell<State>>, text: &str) -> bool {
    if world_namespace() != WorldNamespace::PersonalPlanet {
        return false;
    }
    let text = text
        .trim()
        .trim_end_matches(['.', '!', '?'])
        .to_ascii_lowercase();
    let regenerate = matches!(text.as_str(), "generate courtyard" | "regenerate courtyard");
    let change = if regenerate {
        None
    } else {
        match PlanetChange::from_request(&text) {
            None => return false,
            Some(Ok(change)) => Some(change),
            Some(Err(_)) => {
                planet_note("Say Set time to followed by an hour from 0 to under 24.");
                return true;
            }
        }
    };
    let weak = Rc::downgrade(rc);
    spawn_local(async move {
        if let Err(e) = prepare_planet(weak, regenerate, change).await {
            planet_note(
                e.as_string()
                    .as_deref()
                    .unwrap_or("The courtyard could not change. Your working world is still here."),
            );
        }
    });
    true
}
pub(super) fn mount(
    rc: &Rc<RefCell<State>>,
    listeners: &mut Vec<(EventTarget, &'static str, Handler)>,
) -> Result<(), JsValue> {
    hide("project-tools", false);
    if world_namespace() == WorldNamespace::PersonalPlanet {
        for (id, regenerate) in [("planet-generate", true), ("planet-apply", false)] {
            let weak = Rc::downgrade(rc);
            listen(listeners, id, "click", move |_| {
                let weak = weak.clone();
                spawn_local(async move {
                    if let Err(e) = prepare_planet(weak, regenerate, None).await {
                        planet_note(
                            e.as_string()
                                .as_deref()
                                .unwrap_or("The courtyard could not change. Try again."),
                        );
                    }
                });
            })?;
        }
    }
    history::mount(rc, listeners)?;
    publishing::mount(rc, listeners)?;
    hide("project-save", false);
    if world_namespace() == WorldNamespace::PersonalPlanet {
        for id in [
            "project-list",
            "project-create",
            "project-name",
            "project-picker",
        ] {
            hide(id, true);
        }
    }
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
            change_project(&mut s, picker().value());
            s.projects.images.clear();
            materials::clear(&mut s);
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
    history::poll(rc);
    publishing::poll(rc);
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
        let result = async {
            let document = exported(&weak).await?;
            let rc = weak.upgrade().ok_or(JsValue::NULL)?;
            if !same(&rc.borrow(), &account, epoch) {
                return Err(JsValue::NULL);
            }
            files::collect(&weak, &document).await?;
            Ok::<_, JsValue>(document)
        }
        .await;
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
    if world_namespace() == WorldNamespace::PersonalPlanet {
        if let Ok(d) = serde_json::from_value::<ProjectDocument>(action["document"].clone()) {
            if let Ok(recipe) = PlanetRecipe::from_document(&d) {
                planet_settings(&recipe);
                s.projects.planet_recipe = Some(recipe);
                planet_note(
                    "Your courtyard is ready. Walk, inspect, or save it to your planet project.",
                );
            }
        }
    }
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

pub(super) fn record_materials(s: &mut State, action: &Value) {
    if let Ok(action) = serde_json::from_value::<Action>(action.clone()) {
        match action {
            Action::ApplyMaterial { material } => files::remember(s, &material),
            Action::RestoreMaterialProject { materials, .. } => {
                for material in materials {
                    files::remember(s, &material)
                }
            }
            _ => {}
        }
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
    materials::clear(s);
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
