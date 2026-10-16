//! Private image operations for an immutable Environment attempt. They never
//! allocate, wake, or adopt a computer and never turn a private checkpoint into
//! a reusable image.
use crate::{Store, api::Api, registry::Drivers, store::Desired};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use computer_driver::{Driver, Outcome};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

pub use computer_driver::{EnvironmentAttempt as Attempt, EnvironmentRole as Role};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Capture,
    Read,
    Hydrated,
    Delete,
}

pub(crate) async fn effect<D: Drivers>(
    store: &Store,
    drivers: &D,
    owner: &str,
    id: &str,
    op: Operation,
) -> Outcome<Value> {
    let entry = match store.get(owner, id).await {
        Ok(Some(entry)) => entry,
        Ok(None) => return Outcome::failed("No such computer."),
        Err(_) => return Outcome::unknown("The computer couldn't be read."),
    };
    let Some(attempt) = entry
        .binding
        .as_ref()
        .and_then(|b| b.environment_attempt.as_ref())
    else {
        return Outcome::failed("This computer cannot publish an Environment image.");
    };
    if attempt.validate().is_err() {
        return Outcome::failed("The Environment attempt is invalid.");
    }
    // Deletion clears the active resource accessor, but image ownership must
    // still refer to the exact builder that created it.
    let source = entry
        .record
        .creates
        .iter()
        .rev()
        .find_map(|create| create.resource.as_deref());
    let driver = match drivers.resolve(&entry) {
        Ok(driver) => driver,
        Err(_) => return Outcome::unknown("The computer connection is unavailable."),
    };
    if matches!(op, Operation::Capture)
        && (crate::reconcile::now_ms() >= attempt.deadline_ms
            || !driver.capabilities().environment_artifacts
            || attempt.role != Role::Builder
            || entry.desired != Desired::Stopped
            || entry.phase != "stopped")
    {
        return Outcome::failed("Only a stopped clean builder can publish this image.");
    }
    if matches!(op, Operation::Hydrated) {
        if attempt.role != Role::Verifier {
            return Outcome::failed("This computer isn't a fresh verifier.");
        }
        let Some(resource) = entry.record.resource() else {
            return Outcome::done(json!(false));
        };
        return encode(driver.environment_image_hydrated(resource).await);
    }
    if attempt.role != Role::Builder {
        return Outcome::failed("This attempt does not own an output image.");
    }
    let Some(name) = attempt.image_name.as_deref() else {
        return Outcome::failed("The image name is missing.");
    };
    if matches!(op, Operation::Delete) {
        // The retained builder and its exact connection own this name even
        // after its computer was deleted or the Environment was withdrawn.
        return match driver.read_environment_image(name).await {
            Outcome::Done { value: None } => Outcome::done(json!(false)),
            Outcome::Done { value: Some(image) } if Some(image.source.as_str()) == source => {
                encode(driver.delete_environment_image(name).await)
            }
            Outcome::Done { value: Some(_) } => {
                Outcome::failed("This image belongs to another builder.")
            }
            Outcome::Unknown { reason } | Outcome::Failed { reason } => Outcome::unknown(reason),
        };
    }
    let result = match op {
        Operation::Capture => {
            let Some(resource) = entry.record.resource() else {
                return Outcome::unknown("The builder has no computer yet.");
            };
            match driver.capture_environment_image(resource, name).await {
                Outcome::Done { value } => Outcome::done(Some(value)),
                Outcome::Failed { reason } => Outcome::failed(reason),
                Outcome::Unknown { reason } => Outcome::unknown(reason),
            }
        }
        Operation::Read => driver.read_environment_image(name).await,
        _ => unreachable!(),
    };
    match result {
        Outcome::Done {
            value: Some(mut image),
        } => {
            if image.name != name || Some(image.source.as_str()) != source {
                return Outcome::failed("This image belongs to another builder.");
            }
            image.source = entry.id;
            encode(Outcome::done(Some(image)))
        }
        other => encode(other),
    }
}
fn encode<T: Serialize>(result: Outcome<T>) -> Outcome<Value> {
    match result {
        Outcome::Done { value } => {
            Outcome::done(serde_json::to_value(value).expect("artifact result encodes"))
        }
        Outcome::Failed { reason } => Outcome::failed(reason),
        Outcome::Unknown { reason } => Outcome::unknown(reason),
    }
}
async fn run(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(op): Json<Operation>,
) -> Response {
    let owner = match crate::api::account(&headers) {
        Ok(owner) => owner,
        Err(r) => return r,
    };
    let Some(client) = &api.files else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let result = client.artifact(owner, id, op).await;
    let mut response = Json(result).into_response();
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
}
pub fn routes() -> Router<Arc<Api>> {
    Router::new().route("/v1/computers/{id}/environment-image", post(run))
}
