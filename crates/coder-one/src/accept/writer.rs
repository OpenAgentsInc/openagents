//! Who writes the tests: a Microluna session confined to the suite
//! directory, or, in tests, a script.

use std::path::{Path, PathBuf};

use microluna::{Brief, Config, Ending, Isolation, Transport};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// What one writing session did and cost.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Written {
    /// How it ended: `finished`, `stopped`, `turn_limit`, `deadline`, or
    /// `transport`.
    pub ending: String,
    /// Its finish summary, or why there is none.
    pub summary: String,
    pub turns: usize,
    pub calls: usize,
    /// Microluna's list-price cost.
    pub usd: f64,
    pub milliseconds: u64,
    /// The session's own ATIF trace, when one was written.
    pub trace: Option<PathBuf>,
    /// When the session started, in milliseconds since the Unix epoch, so
    /// a reader can line sessions up and see which overlapped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<u64>,
    /// The session's name, such as `accept-writer-1-2`, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Input tokens across its requests, cached ones included.
    #[serde(default)]
    pub input_tokens: u64,
    /// The input tokens the provider served from its cache.
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

/// Writes tests into a suite directory.
pub trait Writer {
    /// The writer, for the record.
    fn describe(&self) -> Value;

    /// Runs one writing session on `brief` in `suite_dir`; `round` counts
    /// from 1.
    fn write(&self, brief: &Brief, suite_dir: &Path, round: u32) -> impl Future<Output = Written>;

    /// Runs one of several writing sessions that work at the same time:
    /// `name` names its trace, and `directive` says what it works on. By
    /// default, [`Writer::write`].
    fn write_as(
        &self,
        brief: &Brief,
        suite_dir: &Path,
        round: u32,
        name: &str,
        directive: &str,
    ) -> impl Future<Output = Written> {
        let _ = (name, directive);
        self.write(brief, suite_dir, round)
    }

    /// Runs a short, targeted repair session on `brief`, within `turns`
    /// model requests when set. By default, [`Writer::write`].
    fn repair(
        &self,
        brief: &Brief,
        suite_dir: &Path,
        round: u32,
        turns: Option<usize>,
    ) -> impl Future<Output = Written> {
        let _ = turns;
        self.write(brief, suite_dir, round)
    }
}

/// A Microluna session whose workspace is the suite directory: its file
/// tools can't leave it, and its commands run in a writing boundary whose
/// only writable checkout is it.
pub struct MicrolunaWriter<'a, T: Transport> {
    pub transport: &'a T,
    pub config: Config,
    /// How the session's commands are confined. `Boundary` keeps them to
    /// the suite directory; `TaskContainer` trusts the container.
    pub isolation: Isolation,
    /// Where each session's ATIF trace goes, when anywhere.
    pub traces: Option<PathBuf>,
    /// Print each step to standard error.
    pub echo: bool,
}

impl<T: Transport> Writer for MicrolunaWriter<'_, T> {
    fn describe(&self) -> Value {
        json!({
            "writer": "microluna",
            "model": self.config.model,
            "effort": self.config.effort,
            "max_turns": self.config.max_turns,
            "deadline_sec": self.config.deadline.map(|d| d.as_secs()),
            "isolation": self.isolation.word(),
            "instructions": microluna::session::INSTRUCTIONS,
            "guidance_sha256": super::sha256(super::GUIDANCE.as_bytes()),
        })
    }

    async fn write(&self, brief: &Brief, suite_dir: &Path, round: u32) -> Written {
        self.session(
            &self.config,
            brief,
            suite_dir,
            &format!("accept-writer-{round}"),
            "",
        )
        .await
    }

    async fn write_as(
        &self,
        brief: &Brief,
        suite_dir: &Path,
        _round: u32,
        name: &str,
        directive: &str,
    ) -> Written {
        self.session(&self.config, brief, suite_dir, name, directive)
            .await
    }

    async fn repair(
        &self,
        brief: &Brief,
        suite_dir: &Path,
        round: u32,
        turns: Option<usize>,
    ) -> Written {
        let mut config = self.config.clone();
        if let Some(turns) = turns {
            config.max_turns = turns.max(1);
        }
        self.session(
            &config,
            brief,
            suite_dir,
            &format!("accept-writer-{round}"),
            "a targeted repair of the flagged tests",
        )
        .await
    }
}

impl<T: Transport> MicrolunaWriter<'_, T> {
    async fn session(
        &self,
        config: &Config,
        brief: &Brief,
        suite_dir: &Path,
        name: &str,
        directive: &str,
    ) -> Written {
        let started_at_ms = atif::now_ms();
        let mut recorder = microluna::Recorder::new();
        if self.echo {
            recorder = recorder.echoing();
        }
        let mut trace = None;
        if let Some(dir) = &self.traces {
            let path = dir.join(format!("{name}.atif.jsonl"));
            let mut session = atif::Session::opening(
                name,
                &config.model,
                "codex-login",
                &suite_dir.display().to_string(),
                &crate::episode::version(),
            );
            session.directive = directive.to_string();
            if let Ok(log) = atif::Log::create_at(&path, &session) {
                recorder = recorder.logging(log);
                trace = Some(path);
            }
        }
        let workspace = match microluna::Workspace::new(suite_dir) {
            Ok(workspace) => workspace.isolated_by(self.isolation),
            Err(error) => {
                return Written {
                    ending: "transport".to_string(),
                    summary: format!("the suite directory can't be used: {error}"),
                    started_at_ms: Some(started_at_ms),
                    name: Some(name.to_string()),
                    ..Written::default()
                };
            }
        };
        let report = microluna::run(self.transport, &workspace, brief, config, &mut recorder).await;
        recorder.close(match report.ending {
            Ending::Finished => atif::log::ENDED,
            _ => atif::log::INTERRUPTED,
        });
        Written {
            ending: match &report.ending {
                Ending::Finished => "finished",
                Ending::Stopped => "stopped",
                Ending::TurnLimit => "turn_limit",
                Ending::Deadline => "deadline",
                Ending::Transport(_) => "transport",
            }
            .to_string(),
            summary: match (&report.finish, &report.ending) {
                (Some(finish), _) => finish.summary.clone(),
                (None, Ending::Transport(why)) => format!("the session lost its provider: {why}"),
                (None, _) => "the session ended without calling finish".to_string(),
            },
            turns: report.turns,
            calls: report.calls,
            usd: report.cost_usd.unwrap_or_default(),
            milliseconds: report.milliseconds,
            trace,
            started_at_ms: Some(started_at_ms),
            name: Some(name.to_string()),
            input_tokens: report.usage.input,
            cached_tokens: report.usage.cached,
            output_tokens: report.usage.output,
        }
    }
}
