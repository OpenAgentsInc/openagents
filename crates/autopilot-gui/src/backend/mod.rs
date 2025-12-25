use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use autopilot::metrics::{MetricsDb, SessionMetrics, SummaryStats};
use autopilot::parallel::AgentInfo;

#[derive(Clone, Debug)]
pub enum BackendEvent {
    Metrics {
        sessions: Vec<SessionMetrics>,
        summary: SummaryStats,
    },
    Logs {
        path: Option<PathBuf>,
        session_id: Option<String>,
        lines: Vec<String>,
    },
    Agents {
        agents: Vec<AgentInfo>,
    },
    Status {
        message: String,
    },
}

#[derive(Clone, Debug)]
pub struct BackendConfig {
    pub metrics_path: PathBuf,
    pub logs_dir: PathBuf,
    pub refresh_interval: Duration,
    pub agents_interval: Duration,
    pub max_log_lines: usize,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            metrics_path: PathBuf::from("autopilot-metrics.db"),
            logs_dir: PathBuf::from("docs/logs"),
            refresh_interval: Duration::from_secs(2),
            agents_interval: Duration::from_secs(5),
            max_log_lines: 300,
        }
    }
}

pub struct BackendHandle {
    pub receiver: Receiver<BackendEvent>,
}

pub fn start_backend(config: BackendConfig) -> BackendHandle {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || run_backend_loop(config, tx));

    BackendHandle { receiver: rx }
}

fn run_backend_loop(config: BackendConfig, tx: mpsc::Sender<BackendEvent>) {
    let mut metrics_db = None;
    let mut last_agents = Instant::now() - config.agents_interval;

    let runtime = tokio::runtime::Runtime::new().ok();

    loop {
        if metrics_db.is_none() {
            match MetricsDb::open(&config.metrics_path) {
                Ok(db) => metrics_db = Some(db),
                Err(err) => {
                    let _ = tx.send(BackendEvent::Status {
                        message: format!(
                            "Metrics database unavailable: {}",
                            err,
                        ),
                    });
                }
            }
        }

        if let Some(db) = metrics_db.as_ref() {
            let sessions = db.get_recent_sessions(200).unwrap_or_default();
            let summary = db.get_summary_stats().unwrap_or_default();
            let _ = tx.send(BackendEvent::Metrics { sessions, summary });
        }

        let (log_path, log_session, log_lines) = load_latest_log(&config.logs_dir, config.max_log_lines);
        let _ = tx.send(BackendEvent::Logs {
            path: log_path,
            session_id: log_session,
            lines: log_lines,
        });

        if last_agents.elapsed() >= config.agents_interval {
            if let Some(runtime) = runtime.as_ref() {
                match runtime.block_on(autopilot::parallel::list_agents()) {
                    Ok(agents) => {
                        let _ = tx.send(BackendEvent::Agents { agents });
                    }
                    Err(err) => {
                        let _ = tx.send(BackendEvent::Status {
                            message: format!("Parallel agents unavailable: {}", err),
                        });
                    }
                }
            }
            last_agents = Instant::now();
        }

        thread::sleep(config.refresh_interval);
    }
}

fn load_latest_log(logs_dir: &Path, max_lines: usize) -> (Option<PathBuf>, Option<String>, Vec<String>) {
    let path = find_latest_log(logs_dir);
    let Some(path) = path else {
        return (None, None, Vec::new());
    };

    let session_id = autopilot::extract_session_id_from_rlog(&path)
        .ok()
        .and_then(|id| id);

    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(_) => return (Some(path), session_id, Vec::new()),
    };

    let content = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }

    (Some(path), session_id, lines)
}

fn find_latest_log(logs_dir: &Path) -> Option<PathBuf> {
    let mut dirs: Vec<_> = std::fs::read_dir(logs_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .collect();

    dirs.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    let latest_dir = dirs.first()?.path();

    let mut logs: Vec<_> = std::fs::read_dir(&latest_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().map(|ext| ext == "rlog").unwrap_or(false))
        .collect();

    logs.sort_by(|a, b| {
        let a_time = a.metadata().and_then(|m| m.modified()).ok();
        let b_time = b.metadata().and_then(|m| m.modified()).ok();
        b_time.cmp(&a_time)
    });

    logs.first().map(|entry| entry.path())
}
