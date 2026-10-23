//! Creator-owned face images in private project storage and editable downloads.
use super::*;
use std::collections::BTreeSet;
fn unsupported() -> JsValue {
    JsValue::from_str(
        "This saved material needs its matching source images. Keep your exported files.",
    )
}
pub(super) fn remember(s: &mut State, bundle: &MaterialBundle) {
    if bundle.validate().is_err() {
        return;
    }
    if s.projects.images.len() > 256 {
        s.projects.images.clear();
    }
    for image in [&bundle.generated, &bundle.handmade].into_iter().flatten() {
        s.projects
            .images
            .insert(image.resource.sha256.clone(), image.clone());
    }
}
pub(super) fn cached(
    s: &State,
    document: &ProjectDocument,
) -> Result<MaterialProjectArchive, JsValue> {
    let mut materials = Vec::new();
    for material in project_materials(document).map_err(|_| unsupported())? {
        let image = |layer: &ColorLayer| -> Result<Option<MaterialImage>, JsValue> {
            let Some(resource) = &layer.image else {
                return Ok(None);
            };
            s.projects
                .images
                .get(&resource.sha256)
                .filter(|image| &image.resource == resource)
                .cloned()
                .map(Some)
                .ok_or_else(unsupported)
        };
        let generated = image(&material.generated)?;
        let handmade = image(&material.handmade)?;
        materials.push(MaterialBundle {
            document: material,
            generated,
            handmade,
        });
    }
    let archive = MaterialProjectArchive {
        schema: "openagents.verse.project-export.v2".into(),
        document: document.clone(),
        materials,
    };
    archive.validate().map_err(|_| unsupported())?;
    Ok(archive)
}
pub(super) async fn collect(
    weak: &Weak<RefCell<State>>,
    document: &ProjectDocument,
) -> Result<MaterialProjectArchive, JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    let scope = confirmation_scope(&rc.borrow()).ok_or(JsValue::NULL)?;
    let expected = project_materials(document).map_err(|_| unsupported())?;
    let hashes: BTreeSet<_> = expected
        .iter()
        .flat_map(|d| [d.generated.image.as_ref(), d.handmade.image.as_ref()])
        .flatten()
        .map(|r| r.sha256.clone())
        .collect();
    rc.borrow_mut()
        .projects
        .images
        .retain(|hash, _| hashes.contains(hash));
    for material in expected {
        require_scope(weak, &scope)?;
        let known = [&material.generated, &material.handmade]
            .iter()
            .all(|layer| {
                layer.image.as_ref().is_none_or(|r| {
                    rc.borrow()
                        .projects
                        .images
                        .get(&r.sha256)
                        .is_some_and(|i| &i.resource == r)
                })
            });
        if known {
            continue;
        }
        let outcome = materials::detail(
            weak,
            Action::InspectMaterial {
                target: material.target.clone(),
            },
        )
        .await?;
        require_scope(weak, &scope)?;
        let Some(Detail::Material { material: bundle }) = outcome.detail else {
            return Err(unsupported());
        };
        if bundle.document != material {
            return Err(JsValue::from_str(
                "The face material changed. Export or review Save project again.",
            ));
        }
        remember(&mut rc.borrow_mut(), &bundle);
    }
    require_scope(weak, &scope)?;
    cached(&rc.borrow(), document)
}
async fn resource(
    weak: &Weak<RefCell<State>>,
    scope: &ConfirmationScope,
    project: &str,
    image: &ProjectResource,
    upload: Option<&MaterialImage>,
) -> Result<Option<MaterialImage>, JsValue> {
    require_scope(weak, scope)?;
    let init = RequestInit::new();
    init.set_credentials(RequestCredentials::SameOrigin);
    init.set_method(if upload.is_some() { "POST" } else { "GET" });
    if let Some(upload) = upload {
        let bytes = upload.validate().map_err(|_| unsupported())?;
        init.set_body(&js_sys::Uint8Array::from(bytes.as_slice()));
    }
    let request = Request::new_with_str_and_init(
        &format!(
            "/v1/verse/studio/{}/resources/{}",
            project_path(project),
            image.sha256
        ),
        &init,
    )?;
    if upload.is_some() {
        let rc = weak.upgrade().ok_or(JsValue::NULL)?;
        let csrf = rc
            .borrow()
            .csrf
            .clone()
            .ok_or_else(|| JsValue::from_str("Sign in before saving your face layers."))?;
        request.headers().set("x-verse-csrf", &csrf)?;
        request.headers().set("content-type", "image/png")?;
    }
    let response: Response = JsFuture::from(window().fetch_with_request(&request))
        .await?
        .unchecked_into();
    require_scope(weak, scope)?;
    if !response.ok() {
        return Err(JsValue::from_str(
            "The face image could not save or open. Your editable sources are still available to export.",
        ));
    }
    if upload.is_some() {
        let value = JsFuture::from(response.json()?).await?;
        require_scope(weak, scope)?;
        let value: Value = serde_json::from_str(
            &js_sys::JSON::stringify(&value)?
                .as_string()
                .ok_or(JsValue::NULL)?,
        )
        .map_err(|_| unsupported())?;
        let accepted: ProjectResource =
            serde_json::from_value(value["resource"].clone()).map_err(|_| unsupported())?;
        if &accepted != image {
            return Err(unsupported());
        }
        return Ok(None);
    }
    let bytes = js_sys::Uint8Array::new(&JsFuture::from(response.array_buffer()?).await?).to_vec();
    require_scope(weak, scope)?;
    let received = MaterialImage::new(&bytes).map_err(|_| unsupported())?;
    if &received.resource != image {
        return Err(unsupported());
    }
    Ok(Some(received))
}
pub(super) async fn upload(
    weak: &Weak<RefCell<State>>,
    project: &str,
    archive: &MaterialProjectArchive,
) -> Result<(), JsValue> {
    archive.validate().map_err(|_| unsupported())?;
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    let scope = confirmation_scope(&rc.borrow()).ok_or(JsValue::NULL)?;
    let mut done = BTreeSet::new();
    for image in archive
        .materials
        .iter()
        .flat_map(|b| [b.generated.as_ref(), b.handmade.as_ref()])
        .flatten()
    {
        if done.insert(image.resource.sha256.clone()) {
            resource(weak, &scope, project, &image.resource, Some(image)).await?;
        }
    }
    require_scope(weak, &scope)
}
pub(super) async fn restore(
    weak: &Weak<RefCell<State>>,
    project: &str,
    document: ProjectDocument,
    checkpoint: Option<Checkpoint>,
) -> Result<Action, JsValue> {
    let rc = weak.upgrade().ok_or(JsValue::NULL)?;
    let scope = confirmation_scope(&rc.borrow()).ok_or(JsValue::NULL)?;
    let documents = project_materials(&document).map_err(|_| unsupported())?;
    if documents.is_empty() {
        return Ok(Action::RestoreProject {
            document,
            checkpoint,
        });
    }
    let mut materials = Vec::new();
    for document in documents {
        let mut images = Vec::new();
        for layer in [&document.generated, &document.handmade] {
            let image = if let Some(reference) = &layer.image {
                resource(weak, &scope, project, reference, None).await?
            } else {
                None
            };
            images.push(image);
        }
        let material = MaterialBundle {
            document,
            generated: images.remove(0),
            handmade: images.remove(0),
        };
        material.validate().map_err(|_| unsupported())?;
        require_scope(weak, &scope)?;
        remember(&mut rc.borrow_mut(), &material);
        materials.push(material);
    }
    require_scope(weak, &scope)?;
    MaterialProjectArchive {
        schema: "openagents.verse.project-export.v2".into(),
        document,
        materials,
    }
    .restore(checkpoint)
    .map_err(|_| unsupported())
}
pub(super) async fn saved_archive(
    weak: &Weak<RefCell<State>>,
    project: &str,
    document: ProjectDocument,
) -> Result<MaterialProjectArchive, JsValue> {
    let action = restore(weak, project, document, None).await?;
    match action {
        Action::RestoreProject { document, .. } => Ok(MaterialProjectArchive {
            schema: "openagents.verse.project-export.v2".into(),
            document,
            materials: vec![],
        }),
        Action::RestoreMaterialProject {
            document,
            materials,
            ..
        } => Ok(MaterialProjectArchive {
            schema: "openagents.verse.project-export.v2".into(),
            document,
            materials,
        }),
        _ => Err(unsupported()),
    }
}
pub(super) fn download(archive: &MaterialProjectArchive) -> Result<(), JsValue> {
    archive.validate().map_err(|_| unsupported())?;
    let text = if archive.materials.is_empty() {
        serde_json::to_string_pretty(&archive.document)
    } else {
        serde_json::to_string_pretty(archive)
    }
    .map_err(|_| JsValue::NULL)?;
    let a = window().document().unwrap().create_element("a")?;
    a.set_attribute(
        "href",
        &format!(
            "data:application/json;charset=utf-8,{}",
            js_sys::encode_uri_component(&text)
        ),
    )?;
    a.set_attribute("download", "openagents-world.json")?;
    a.unchecked_into::<web_sys::HtmlElement>().click();
    Ok(())
}
