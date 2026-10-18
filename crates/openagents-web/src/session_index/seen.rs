//! Per-viewer read markers for authorized indexed sessions. No Conversation ID joins.
use super::*;
use sha2::{Digest, Sha256};

const KEY: &str = "sessions/seen.json";
const MAX_MARKERS: usize = 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Marker {
    revision: String,
    touched: u64,
}
pub(super) type Markers = BTreeMap<String, Marker>;
pub(super) fn identity(owner: &str, row: &Row) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!("{owner}\0{}\0{}", row.device, row.id))
    )
}
fn revision(row: &Row) -> Result<String, Error> {
    let bytes = serde_json::to_vec(row)
        .map_err(|_| Error::Unavailable("The session could not be read."))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
pub(super) async fn load(store: &Store, viewer: &str) -> Result<(Markers, Option<String>), Error> {
    match store.read_key(&Store::owner_key(viewer, KEY)?).await? {
        Some((bytes, generation)) => {
            if bytes.len() > 512 * 1024 {
                return Err(Error::Unavailable("The read history could not be read."));
            }
            let markers: Markers = serde_json::from_slice(&bytes)
                .map_err(|_| Error::Unavailable("The read history could not be read."))?;
            if markers.len() > MAX_MARKERS {
                return Err(Error::Unavailable("The read history could not be read."));
            }
            Ok((markers, Some(generation)))
        }
        None => Ok((Markers::new(), None)),
    }
}
pub(super) fn decorate(
    value: &mut Value,
    markers: &Markers,
    owner: &str,
    row: &Row,
) -> Result<(), Response> {
    let revision = revision(row).map_err(|e| stored(&e))?;
    let unseen = markers
        .get(&identity(owner, row))
        .is_none_or(|m| m.revision != revision);
    value["record_key"] = json!(identity(owner, row));
    value["activity_revision"] = json!(revision);
    value["unseen"] = json!(unseen);
    Ok(())
}
fn remember(markers: &mut Markers, id: String, revision: String, touched: u64) {
    markers.insert(id, Marker { revision, touched });
    while markers.len() > MAX_MARKERS {
        let oldest = markers
            .iter()
            .min_by(|(ak, a), (bk, b)| a.touched.cmp(&b.touched).then_with(|| ak.cmp(bk)))
            .map(|(id, _)| id.clone());
        if let Some(id) = oldest {
            markers.remove(&id);
        } else {
            break;
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    activity_revision: String,
}
/// Mark only the revision actually shown, after normal owned/shared authorization.
pub(super) async fn mark(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(scope): Query<Scope>,
    body: Bytes,
) -> Response {
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        mark_inner(app, headers, id, scope, body),
    )
    .await
    {
        Ok(response) => response,
        Err(_) => refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "Couldn't update this chat. Try again.",
        ),
    }
}
async fn mark_inner(
    app: App,
    headers: HeaderMap,
    id: String,
    scope: Scope,
    body: Bytes,
) -> Response {
    let viewer = match crate::coder_sync::owner(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if scope
        .record
        .as_deref()
        .is_none_or(|r| r.len() != 64 || !r.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return invalid("Open the chat again.");
    }
    if body.len() > 256 {
        return invalid("Open the chat again.");
    }
    let Ok(request) = serde_json::from_slice::<Request>(&body) else {
        return invalid("Open the chat again.");
    };
    if request.activity_revision.len() != 64
        || !request
            .activity_revision
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return invalid("Open the chat again.");
    }
    let found = match find_record(
        &app,
        &headers,
        &viewer,
        &id,
        scope.project.as_deref(),
        scope.record.as_deref(),
    )
    .await
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    let current = match revision(found.row()) {
        Ok(v) => v,
        Err(e) => return stored(&e),
    };
    if current != request.activity_revision {
        return refused(
            StatusCode::CONFLICT,
            "activity_changed",
            "The chat changed. Open it again.",
        );
    }
    let key = match Store::owner_key(&viewer, KEY) {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    for _ in 0..6 {
        let (mut markers, generation) = match load(&app.config.chat_store, &viewer).await {
            Ok(v) => v,
            Err(e) => return stored(&e),
        };
        remember(
            &mut markers,
            identity(&found.owner, found.row()),
            current.clone(),
            now_unix(),
        );
        let bytes = match serde_json::to_vec(&markers) {
            Ok(b) => b,
            Err(_) => return invalid("The read history could not be saved."),
        };
        match app
            .config
            .chat_store
            .write_key(&key, bytes, generation.as_deref())
            .await
        {
            Ok(_) => {
                return answer(
                    StatusCode::OK,
                    json!({"id":id,"record_key":identity(&found.owner,found.row()),"activity_revision":current,"unseen":false}),
                );
            }
            Err(Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    stored(&Error::Conflict)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn row() -> Row {
        serde_json::from_value(json!({"id":"00000000-0000-0000-0000-000000000001","device":"device-a","computer":"Synthetic","title":"Example","status":"idle","last_activity_unix":10})).unwrap()
    }
    #[test]
    fn read_marker_is_bound_to_owner_device_and_exact_published_activity() {
        let a = row();
        let mut markers = Markers::new();
        let mut value = json!({});
        decorate(&mut value, &markers, "owner-a", &a).unwrap();
        assert_eq!(value["unseen"], true);
        remember(
            &mut markers,
            identity("owner-a", &a),
            revision(&a).unwrap(),
            10,
        );
        decorate(&mut value, &markers, "owner-a", &a).unwrap();
        assert_eq!(value["unseen"], false);
        decorate(&mut value, &markers, "owner-b", &a).unwrap();
        assert_eq!(value["unseen"], true);
        let mut device = a.clone();
        device.device = "device-b".into();
        assert_ne!(identity("owner-a", &device), identity("owner-a", &a));
        let mut next = a.clone();
        next.last_activity_unix += 1;
        decorate(&mut value, &markers, "owner-a", &next).unwrap();
        assert_eq!(value["unseen"], true);
        next = a.clone();
        next.status = Status::Working;
        assert_ne!(revision(&next).unwrap(), revision(&a).unwrap());
    }
    #[test]
    fn marker_record_is_bounded_with_deterministic_eviction() {
        let mut markers = Markers::new();
        for n in 0..MAX_MARKERS + 10 {
            remember(
                &mut markers,
                format!("{n:05}"),
                "synthetic".into(),
                n as u64,
            );
        }
        assert_eq!(markers.len(), MAX_MARKERS);
        assert!(!markers.contains_key("00000"));
        assert!(markers.contains_key("01033"));
    }
}
