//! `POST /v1/verse/house/request`: one bounded scene decision for a Haunted
//! House request the browser's local phrases didn't cover.
//!
//! The decision is the original house editor's: [`kiln::director::decide`],
//! one Jev System One request with typed choices (action, asset, placement,
//! size) and a 0.75 confidence floor. No text or numbers the model writes
//! become edits; positions come from the player's pose in
//! [`aethyrforge_host::house_request`], and only the house's own listed
//! assets can appear. Guests may call it; it changes nothing on the server.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use aethyrforge_host::house_request::{
    self, HouseAnswer, HouseRequest, Intent, NOT_UNDERSTOOD, Placement, Size,
};
use axum::{
    Router,
    extract::{ConnectInfo, Request},
    http::StatusCode,
    response::Response,
    routing::post,
};
use kiln::director::{self, Action, Decision};

use crate::{App, coder_sync::refused};

pub(crate) const PATH: &str = house_request::ENDPOINT;
/// The server's whole budget for one request; the browser waits 5 s.
const DEADLINE: Duration = Duration::from_secs(4);
const MAX_BODY: usize = 32 * 1024;
/// Requests per address per minute.
const PER_MINUTE: u32 = 20;

pub(crate) fn routes() -> Router<App> {
    Router::new().route(PATH, post(handle))
}

fn client() -> Option<&'static jev::Client> {
    static CLIENT: OnceLock<Option<jev::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| match director::client() {
            Ok(c) => Some(c),
            Err(error) => {
                eprintln!("openagents-web: house requests: no decision client: {error}");
                None
            }
        })
        .as_ref()
}

fn allowed(ip: IpAddr, now: Instant) -> bool {
    static SEEN: OnceLock<Mutex<HashMap<IpAddr, (Instant, u32)>>> = OnceLock::new();
    let Ok(mut seen) = SEEN.get_or_init(Default::default).lock() else {
        return false;
    };
    if seen.len() > 50_000 {
        seen.retain(|_, (at, _)| now.duration_since(*at) < Duration::from_secs(60));
    }
    let entry = seen.entry(ip).or_insert((now, 0));
    if now.duration_since(entry.0) >= Duration::from_secs(60) {
        *entry = (now, 0);
    }
    entry.1 += 1;
    entry.1 <= PER_MINUTE
}

fn answer(status: StatusCode, body: &HouseAnswer) -> Response {
    crate::coder_sync::answer(status, serde_json::to_value(body).unwrap_or_default())
}

async fn handle(request: Request) -> Response {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    let ip = crate::cloud::magic_email::client_ip(
        peer,
        request.headers(),
        std::env::var_os("K_SERVICE").is_some(),
    );
    if !allowed(ip, Instant::now()) {
        return refused(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "That's a lot of requests at once. Wait a moment and try again.",
        );
    }
    let Ok(bytes) = axum::body::to_bytes(request.into_body(), MAX_BODY).await else {
        return refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "too_large",
            "Describe one change in a short request.",
        );
    };
    let Ok(sent) = serde_json::from_slice::<HouseRequest>(&bytes) else {
        return refused(StatusCode::BAD_REQUEST, "invalid", NOT_UNDERSTOOD);
    };
    if let Err(message) = sent.validate() {
        return refused(StatusCode::BAD_REQUEST, "invalid", message);
    }
    let started = Instant::now();
    let result = respond(&sent, async {
        let client = client().ok_or_else(|| "no decision client".to_string())?;
        director::decide(
            client,
            &director::Request {
                text: sent.text.clone(),
                selected: sent.selected.is_some(),
                lab: Default::default(),
            },
        )
        .await
    })
    .await;
    eprintln!(
        "openagents-web: house request {} in {} ms",
        if result.is_ok() {
            "answered"
        } else {
            "refused"
        },
        started.elapsed().as_millis()
    );
    match result {
        Ok(body) => answer(StatusCode::OK, &body),
        Err((status, message)) => refused(status, "not_understood", message),
    }
}

/// Bound `decision` by [`DEADLINE`] and turn it into edits for `sent`.
pub(crate) async fn respond(
    sent: &HouseRequest,
    decision: impl std::future::Future<Output = Result<Decision, String>>,
) -> Result<HouseAnswer, (StatusCode, &'static str)> {
    // A local phrase never needs the model, even when the browser asked.
    let assets = house_request::house_assets(&sent.assets);
    if let Some(intent) = house_request::parse(&sent.text, &assets) {
        return house_request::answer(&intent, &sent.player, sent.selected.as_deref())
            .map_err(|m| (StatusCode::UNPROCESSABLE_ENTITY, m));
    }
    let decision = match tokio::time::timeout(DEADLINE, decision).await {
        Err(_) => {
            return Err((
                StatusCode::GATEWAY_TIMEOUT,
                "Couldn't do that in time. Try 'add three pumpkins to my left'.",
            ));
        }
        Ok(Err(_)) => return Err((StatusCode::SERVICE_UNAVAILABLE, NOT_UNDERSTOOD)),
        Ok(Ok(d)) => d,
    };
    let steps: Vec<&Decision> = if decision.steps.is_empty() {
        vec![&decision]
    } else {
        decision.steps.iter().map(|s| s.decision.as_ref()).collect()
    };
    let mut out = HouseAnswer {
        edits: Vec::new(),
        say: String::new(),
    };
    for (n, step) in steps.iter().enumerate() {
        let text = if decision.steps.is_empty() {
            sent.text.as_str()
        } else {
            decision.steps[n].text.as_str()
        };
        let intent = intent(step, text, &assets, sent.selected.as_deref())
            .ok_or((StatusCode::UNPROCESSABLE_ENTITY, NOT_UNDERSTOOD))?;
        let part = house_request::answer(&intent, &sent.player, sent.selected.as_deref())
            .map_err(|m| (StatusCode::UNPROCESSABLE_ENTITY, m))?;
        out.edits.extend(part.edits);
        out.say = if out.say.is_empty() {
            part.say
        } else {
            format!("{} {}", out.say, part.say)
        };
    }
    let history = out
        .edits
        .iter()
        .filter(|e| {
            matches!(
                e,
                house_request::HouseEdit::Undo | house_request::HouseEdit::Redo
            )
        })
        .count();
    if out.edits.is_empty()
        || out.edits.len() > house_request::MAX_COUNT
        || (history > 0 && out.edits.len() > 1)
    {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, NOT_UNDERSTOOD));
    }
    Ok(out)
}

/// The understood request in a typed decision, or `None`.
fn intent(
    d: &Decision,
    text: &str,
    assets: &[aethyrforge_host::CreatorAsset],
    selected: Option<&str>,
) -> Option<Intent> {
    if !d.confidence.is_finite() || d.confidence < director::MIN_CONFIDENCE || d.needs_review {
        return None;
    }
    match d.action {
        Action::AddAsset => {
            let id = format!("{}{}", house_request::HOUSE_ASSET_PREFIX, d.asset);
            let asset = assets.iter().find(|a| a.id == id)?.id.clone();
            let placement = match d.placement.as_str() {
                "left" => Placement::Left,
                "right" => Placement::Right,
                "behind" => Placement::Behind,
                "ahead" | "anywhere" => Placement::Ahead,
                _ => return None,
            };
            let size = match d.size.as_str() {
                "small" => Size::Small,
                "normal" | "exact" => Size::Normal,
                "large" => Size::Large,
                "huge" => Size::Huge,
                _ => return None,
            };
            Some(Intent::Add {
                asset,
                count: house_request::count_in(text)?,
                size,
                placement,
            })
        }
        Action::RemoveAsset if selected.is_some() => Some(Intent::RemoveSelected),
        Action::Undo => Some(Intent::Undo),
        Action::Redo => Some(Intent::Redo),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aethyrforge_host::CreatorAsset;
    use aethyrforge_host::house_request::HouseEdit;

    fn sent(text: &str) -> HouseRequest {
        HouseRequest {
            text: text.into(),
            assets: [
                ("quaternius:fantasy-standard:barrel", "Barrel"),
                ("custom:haunt:pumpkin", "Pumpkin"),
                ("custom:haunt:ghost", "Ghost"),
            ]
            .into_iter()
            .map(|(id, name)| CreatorAsset {
                id: id.into(),
                name: name.into(),
            })
            .collect(),
            player: house_request::Pose {
                position: [0.0, 0.0, 0.0],
                yaw: 0.0,
            },
            selected: None,
        }
    }
    fn decision(
        action: Action,
        asset: &str,
        placement: &str,
        size: &str,
        confidence: f64,
    ) -> Decision {
        Decision {
            action,
            asset: asset.into(),
            asset_resolution: "decision".into(),
            placement: placement.into(),
            size: size.into(),
            tint: "unsupported".into(),
            behavior: "unsupported".into(),
            target_scope: "unsupported".into(),
            confidence,
            needs_review: confidence < director::MIN_CONFIDENCE,
            model: "fixture".into(),
            model_ms: 1.0,
            judgments: serde_json::Value::Null,
            steps: Vec::new(),
        }
    }

    #[tokio::test]
    async fn a_decision_becomes_scaled_edits_beside_the_player() {
        let a = respond(
            &sent("could you maybe drop five really enormous jack o lanterns over on my left side"),
            async { Ok(decision(Action::AddAsset, "pumpkin", "left", "huge", 0.98)) },
        )
        .await
        .unwrap();
        assert_eq!(a.edits.len(), 5);
        for e in &a.edits {
            let HouseEdit::Place {
                asset,
                position,
                scale,
                ..
            } = e
            else {
                panic!("{e:?}")
            };
            assert_eq!(asset, "custom:haunt:pumpkin");
            assert_eq!(*scale, Some(2.5));
            assert!(position[0] < -1.0);
        }
        assert_eq!(a.say, "Added five huge pumpkins to your left.");
    }

    #[tokio::test]
    async fn local_phrases_skip_the_model() {
        let a = respond(&sent("add three pumpkins to my left"), async {
            panic!("no model call")
        })
        .await
        .unwrap();
        assert_eq!(a.edits.len(), 3);
    }

    #[tokio::test]
    async fn unknown_assets_low_confidence_errors_and_timeouts_refuse() {
        // The town barrel is listed, but it is not a haunted-house asset.
        let barrel = respond(&sent("drop a barrel somewhere"), async {
            Ok(decision(
                Action::AddAsset,
                "barrel",
                "anywhere",
                "normal",
                0.99,
            ))
        })
        .await;
        assert_eq!(barrel.unwrap_err().1, NOT_UNDERSTOOD);
        // A catalog kind the house doesn't list.
        let cauldron = respond(&sent("I'd like a cauldron"), async {
            Ok(decision(
                Action::AddAsset,
                "cauldron",
                "ahead",
                "normal",
                0.99,
            ))
        })
        .await;
        assert!(cauldron.is_err());
        let unsure = respond(&sent("spookify it"), async {
            Ok(decision(Action::AddAsset, "ghost", "ahead", "normal", 0.5))
        })
        .await;
        assert!(unsure.is_err());
        let bad = respond(&sent("spookify it"), async {
            Err("The decision response contained an unknown action or argument.".to_string())
        })
        .await;
        assert_eq!(bad.unwrap_err().0, StatusCode::SERVICE_UNAVAILABLE);
        let unsupported = respond(&sent("paint the sky"), async {
            Ok(decision(
                Action::SetTint,
                "unsupported",
                "unsupported",
                "normal",
                0.99,
            ))
        })
        .await;
        assert!(unsupported.is_err());
        let too_many = respond(&sent("I want twelve ghosts ahead"), async {
            Ok(decision(Action::AddAsset, "ghost", "ahead", "normal", 0.99))
        })
        .await;
        assert!(too_many.is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn a_slow_decision_times_out_with_a_clear_message() {
        let slow = respond(&sent("give the hallway some ghosts, please"), async {
            tokio::time::sleep(Duration::from_secs(30)).await;
            Ok(decision(Action::AddAsset, "ghost", "ahead", "normal", 0.99))
        })
        .await;
        let (status, message) = slow.unwrap_err();
        assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
        assert!(message.contains("in time"));
    }

    #[tokio::test]
    async fn the_endpoint_refuses_bad_json_and_floods() {
        let root = tempfile::tempdir().unwrap();
        let app = crate::router(crate::Config::development(root.path().join("tasks")));
        use tower::ServiceExt;
        let local = |body: String| {
            let mut r = axum::http::Request::post(PATH)
                .header(axum::http::header::HOST, "127.0.0.1:4300")
                .header(crate::LOCAL_HEADER, "1")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body))
                .unwrap();
            r.extensions_mut()
                .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 9))));
            r
        };
        let response = app
            .clone()
            .oneshot(local("{not json".into()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = serde_json::to_string(&sent("add a ghost behind me")).unwrap();
        let response = app.clone().oneshot(local(body)).await.unwrap();
        // A local phrase answers even with no decision key on this machine.
        assert_eq!(response.status(), StatusCode::OK);
        let now = Instant::now();
        let ip: IpAddr = "198.51.100.7".parse().unwrap();
        assert!((0..PER_MINUTE).all(|_| allowed(ip, now)));
        assert!(!allowed(ip, now));
        assert!(allowed(ip, now + Duration::from_secs(61)));
    }

    /// The real decision through the server's doors (`OPENROUTER_API_KEY`,
    /// `AI_GATEWAY_API_KEY`, or `TYPESAFE_API_KEY`), timed.
    /// `cargo test -p openagents-web --lib live_house_decision -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "calls the Jev decision service"]
    async fn live_house_decision() {
        let client = client().expect("a decision key");
        for text in [
            "could you drop five really enormous jack o lanterns over on my left side",
            "I want a coffin behind me",
            "put a couple of ghosts right in front of me",
            "take that back",
        ] {
            let mut s = sent(text);
            s.assets.push(CreatorAsset {
                id: "custom:haunt:coffin".into(),
                name: "Coffin".into(),
            });
            let started = Instant::now();
            let result = respond(&s, async {
                director::decide(
                    client,
                    &director::Request {
                        text: text.into(),
                        selected: false,
                        lab: Default::default(),
                    },
                )
                .await
            })
            .await;
            println!(
                "house decision {} ms: {text:?} -> {:?}",
                started.elapsed().as_millis(),
                result.as_ref().map(|a| (&a.say, a.edits.len()))
            );
        }
    }
}
