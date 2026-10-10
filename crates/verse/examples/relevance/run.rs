//! Runs every lane over the same files at the same time and streams each
//! answer back as it arrives. Lanes run in parallel; within a lane, files go
//! one at a time (or `conc` at a time), so a lane's latency is its own.
use super::cases::{Auth, Backend, Case, host_port, order, request};
use std::{
    net::{TcpStream, ToSocketAddrs},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    time::{Duration, Instant},
};

/// What a lane reports.
#[derive(Clone, Debug)]
pub enum Event {
    /// The lane reached its server and is starting.
    Started { lane: usize, at: f64 },
    /// The lane cannot run: unreachable, or no key.
    Offline { lane: usize, why: String },
    /// One file's answer.
    Decision {
        lane: usize,
        file: usize,
        p: f64,
        latency: Duration,
        at: f64,
    },
    /// One file's request failed.
    Failed {
        lane: usize,
        file: usize,
        error: String,
    },
    /// Why a running lane has nothing to show yet, such as a model loading.
    Note { lane: usize, text: String },
    /// The lane has asked about every file.
    Done { lane: usize },
}

/// A run in flight. Dropping it stops every lane and abandons the requests
/// in flight, so a rerun does not queue behind the last run's.
pub struct Run {
    pub events: Receiver<Event>,
    cancel: Arc<AtomicBool>,
}

impl Drop for Run {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// TypeSafe's key: `TYPESAFE_API_KEY`, else `~/work/.secrets/typesafe.env`.
/// The key is never printed.
pub fn typesafe_key() -> Option<String> {
    if let Ok(key) = std::env::var("TYPESAFE_API_KEY")
        && !key.trim().is_empty()
    {
        return Some(key.trim().to_owned());
    }
    let home = std::env::var("HOME").ok()?;
    let text = std::fs::read_to_string(format!("{home}/work/.secrets/typesafe.env")).ok()?;
    text.lines().find_map(|l| {
        let l = l.trim().strip_prefix("export ").unwrap_or(l.trim());
        let v = l.strip_prefix("TYPESAFE_API_KEY=")?;
        let v = v.trim().trim_matches('"').trim_matches('\'');
        (!v.is_empty()).then(|| v.to_owned())
    })
}

fn reachable(base: &str) -> Result<(), String> {
    let hp = host_port(base).ok_or("bad base URL")?;
    let addr = hp
        .to_socket_addrs()
        .map_err(|e| format!("{hp}: {e}"))?
        .next()
        .ok_or_else(|| format!("{hp}: no address"))?;
    TcpStream::connect_timeout(&addr, Duration::from_millis(700))
        .map(|_| ())
        .map_err(|_| format!("nothing listening at {hp}"))
}

/// Whether an Ollama server at `base` has `model` in memory, from its
/// `/api/ps`. None when the server is not Ollama or does not answer. Ollama
/// unloads a model five minutes after its last request, and loading one
/// again takes seconds (Clef-Flash) to over half a minute (Clef 27B).
fn ollama_loaded(base: &str, model: &str) -> Option<bool> {
    use std::io::{Read, Write};
    let hp = host_port(base)?;
    let addr = hp.to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(700)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    write!(stream, "GET /api/ps HTTP/1.0\r\nHost: {hp}\r\n\r\n").ok()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw);
    let (head, body) = text.split_once("\r\n\r\n")?;
    if !head.starts_with("HTTP/1.") || !head.contains(" 200") {
        return None;
    }
    Some(loaded_in_ps(body, model)?)
}

/// Reads `/api/ps`'s model names: `clef` matches `clef:latest`, not
/// `clef-flash:latest`.
fn loaded_in_ps(body: &str, model: &str) -> Option<bool> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let models = v["models"].as_array()?;
    Some(models.iter().any(|m| {
        let name = m["name"].as_str().unwrap_or_default();
        name == model || name.split_once(':').is_some_and(|(n, _)| n == model)
    }))
}

/// Resolves when the run is cancelled.
async fn cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn client(backend: &Backend, timeout: Duration) -> Result<jev::Client, String> {
    let retry = jev::RetryPolicy {
        max_retries: 1,
        ..Default::default()
    };
    let config = match backend.auth {
        Auth::Local => jev::Config::local(backend.base.clone(), backend.model.clone()),
        Auth::TypeSafe => jev::Config::new()
            .api_key(
                typesafe_key()
                    .ok_or("no TYPESAFE_API_KEY (env or ~/work/.secrets/typesafe.env)")?,
            )
            .base_url(backend.base.clone())
            .default_model(backend.model.clone()),
    };
    jev::Client::new(config.timeout(timeout).retry(retry)).map_err(|e| e.to_string())
}

async fn lane(
    index: usize,
    backend: Backend,
    case: Arc<Case>,
    files: Vec<usize>,
    nonce: String,
    conc: usize,
    timeout: Duration,
    t0: Instant,
    tx: Sender<Event>,
    cancel: Arc<AtomicBool>,
) {
    let probe = backend.base.clone();
    let reach = tokio::task::spawn_blocking(move || reachable(&probe))
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
    let client = reach.and_then(|()| client(&backend, timeout));
    let client = match client {
        Ok(c) => Arc::new(c),
        Err(why) => {
            let _ = tx.send(Event::Offline { lane: index, why });
            return;
        }
    };
    let _ = tx.send(Event::Started {
        lane: index,
        at: t0.elapsed().as_secs_f64(),
    });
    if backend.auth == Auth::Local {
        let (base, model) = (backend.base.clone(), backend.model.clone());
        let loaded = tokio::task::spawn_blocking(move || ollama_loaded(&base, &model))
            .await
            .ok()
            .flatten();
        if loaded == Some(false) {
            let _ = tx.send(Event::Note {
                lane: index,
                text: format!("loading {} into Ollama", backend.model),
            });
        }
    }
    let next = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let files = Arc::new(files);
    let workers: Vec<_> = (0..conc.max(1))
        .map(|_| {
            let (client, case, files, next, tx, cancel, nonce, model) = (
                client.clone(),
                case.clone(),
                files.clone(),
                next.clone(),
                tx.clone(),
                cancel.clone(),
                nonce.clone(),
                backend.model.clone(),
            );
            tokio::spawn(async move {
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&file) = files.get(i) else {
                        return;
                    };
                    let req =
                        request(&case.issue, &case.candidates[file], &nonce).model(model.clone());
                    let started = Instant::now();
                    let result = tokio::select! {
                        r = client.system_one(req) => r,
                        () = cancelled(&cancel) => return,
                    };
                    let latency = started.elapsed();
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    let event = match result.and_then(|r| r.noul("relevant").map(|a| a.noul)) {
                        Ok(p) => Event::Decision {
                            lane: index,
                            file,
                            p,
                            latency,
                            at: t0.elapsed().as_secs_f64(),
                        },
                        Err(e) => Event::Failed {
                            lane: index,
                            file,
                            error: e.to_string(),
                        },
                    };
                    let _ = tx.send(event);
                }
            })
        })
        .collect();
    for w in workers {
        let _ = w.await;
    }
    if !cancel.load(Ordering::Relaxed) {
        let _ = tx.send(Event::Done { lane: index });
    }
}

/// Starts every lane on `case`. `seed` fixes the shared file order and the
/// run nonce's tail.
pub fn start(
    case: Arc<Case>,
    backends: Vec<Backend>,
    conc: usize,
    timeout: Duration,
    seed: u64,
) -> Run {
    let (tx, events) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                for lane in 0..backends.len() {
                    let _ = tx.send(Event::Offline {
                        lane,
                        why: e.to_string(),
                    });
                }
                return;
            }
        };
        let files = order(case.candidates.len(), seed);
        let nonce = format!(
            "relevance-viz-{:x}-{seed:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos() as u64)
        );
        let t0 = Instant::now();
        runtime.block_on(async move {
            let lanes: Vec<_> = backends
                .into_iter()
                .enumerate()
                .map(|(i, b)| {
                    tokio::spawn(lane(
                        i,
                        b,
                        case.clone(),
                        files.clone(),
                        nonce.clone(),
                        conc,
                        timeout,
                        t0,
                        tx.clone(),
                        flag.clone(),
                    ))
                })
                .collect();
            for l in lanes {
                let _ = l.await;
            }
        });
    });
    Run { events, cancel }
}

/// The scoreboard: every lane's answers for one case.
pub struct Board {
    pub case: Arc<Case>,
    pub lanes: Vec<super::cases::Lane>,
    pub labels: Vec<Option<bool>>,
}

impl Board {
    pub fn new(case: Arc<Case>, backends: &[Backend]) -> Self {
        let n = case.candidates.len();
        Self {
            labels: case.labels(),
            lanes: backends
                .iter()
                .map(|b| super::cases::Lane::new(b.clone(), n))
                .collect(),
            case,
        }
    }

    /// Lands one event on its lane.
    pub fn apply(&mut self, event: &Event) {
        use super::cases::LaneStatus;
        match event {
            Event::Started { lane, at } => {
                let l = &mut self.lanes[*lane];
                l.status = LaneStatus::Running;
                l.started = Some(*at);
            }
            Event::Offline { lane, why } => {
                self.lanes[*lane].status = LaneStatus::Offline(why.clone());
            }
            Event::Decision {
                lane,
                file,
                p,
                latency,
                at,
            } => {
                let l = &mut self.lanes[*lane];
                l.p[*file] = Some(*p);
                l.latencies.push(*latency);
                l.first.get_or_insert(*at);
                l.last = Some(*at);
                l.note = None;
            }
            Event::Failed { lane, error, .. } => {
                let l = &mut self.lanes[*lane];
                l.errors += 1;
                l.last_error = Some(error.clone());
            }
            Event::Note { lane, text } => self.lanes[*lane].note = Some(text.clone()),
            Event::Done { lane } => self.lanes[*lane].status = LaneStatus::Done,
        }
    }

    /// Every lane is offline or has asked about every file.
    pub fn finished(&self) -> bool {
        use super::cases::LaneStatus;
        self.lanes
            .iter()
            .all(|l| matches!(l.status, LaneStatus::Done | LaneStatus::Offline(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::{Candidate, Issue, LaneStatus, Origin, registry};

    #[test]
    fn the_board_lands_events_on_their_lanes() {
        let case = Arc::new(Case {
            issue: Issue {
                number: 1,
                title: "t".into(),
                body: "b".into(),
                state: "CLOSED".into(),
            },
            fix: Some(crate::cases::Fix {
                commits: vec!["c".into()],
                files: vec!["a.rs".into()],
            }),
            rev: "c^".into(),
            candidates: ["a.rs", "b.rs"]
                .iter()
                .zip([Origin::Fix, Origin::Random])
                .map(|(p, origin)| Candidate {
                    path: (*p).into(),
                    content: String::new(),
                    truncated: false,
                    origin,
                })
                .collect(),
        });
        let backends = registry();
        let mut board = Board::new(case, &backends[..2]);
        assert_eq!(board.labels, [Some(true), Some(false)]);
        board.apply(&Event::Started { lane: 0, at: 0.0 });
        board.apply(&Event::Offline {
            lane: 1,
            why: "down".into(),
        });
        board.apply(&Event::Decision {
            lane: 0,
            file: 1,
            p: 0.2,
            latency: Duration::from_millis(300),
            at: 0.5,
        });
        board.apply(&Event::Failed {
            lane: 0,
            file: 0,
            error: "boom".into(),
        });
        assert!(!board.finished());
        board.apply(&Event::Done { lane: 0 });
        assert!(board.finished());
        let lane = &board.lanes[0];
        assert_eq!((lane.p[1], lane.errors, lane.answered()), (Some(0.2), 1, 1));
        assert_eq!(lane.status, LaneStatus::Done);
        assert_eq!(board.lanes[1].status, LaneStatus::Offline("down".into()));
    }

    #[test]
    fn ollama_ps_names_match_by_model_not_prefix() {
        let ps = r#"{"models":[{"name":"clef-flash:latest"}]}"#;
        assert_eq!(loaded_in_ps(ps, "clef-flash"), Some(true));
        assert_eq!(loaded_in_ps(ps, "clef"), Some(false));
        assert_eq!(loaded_in_ps(r#"{"models":[]}"#, "clef"), Some(false));
        assert_eq!(loaded_in_ps("not json", "clef"), None);
        assert_eq!(ollama_loaded("http://127.0.0.1:1", "clef"), None);
    }

    #[test]
    fn an_unreachable_lane_reports_offline() {
        assert!(reachable("http://127.0.0.1:1").is_err());
        assert!(reachable("nonsense").is_err());
    }
}
