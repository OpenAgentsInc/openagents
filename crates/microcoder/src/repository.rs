//! Opt-in repository execution through the common task owner.
//!
//! This profile runs the existing loop with model-written acceptance, routing,
//! and knowledge explicitly disabled. Independent checks remain host work.

use std::path::Path;
use std::time::Duration;

use atif::{Source, Step};
use coder::task::{self, adapter::Host};
use serde_json::{Value, json};

use crate::env::Env;
use crate::models::{Basis, Generate, Generated, Judge, Judgment, QuestionSet};
use crate::run::{Ending, Event, Limits, Models, Observer, Route};
use crate::state::{CommandResult, State, cut};

pub struct Repository<'a> {
    pub host: &'a Host,
}

impl Env for Repository<'_> {
    async fn run(&self, command: &str, deadline: Duration) -> CommandResult {
        match self.host.command(command, deadline).await {
            Ok(result) => CommandResult {
                command: command.into(),
                exit: result.exit,
                timed_out: result.timed_out,
                seconds: result.seconds,
                output: cut(
                    &result.output,
                    crate::state::OUTPUT_HEAD,
                    crate::state::OUTPUT_TAIL,
                ),
            },
            Err(error) => CommandResult {
                command: command.into(),
                exit: None,
                timed_out: false,
                seconds: 0.0,
                output: format!("The repository host refused the command: {error}"),
            },
        }
    }

    async fn read(&self, path: &str) -> Option<String> {
        let path = if self.host.configuration().container.is_some() {
            Path::new(path)
                .strip_prefix("/workspace")
                .ok()
                .and_then(Path::to_str)
                .unwrap_or(path)
        } else {
            path
        };
        self.host
            .read(path, crate::env::FILE_MAX * 4)
            .ok()
            .flatten()
            .map(|bytes| cut(&String::from_utf8_lossy(&bytes), crate::env::FILE_MAX, 0))
    }
}

fn refused_generation(model: &str, dispatched: bool, reason: &str) -> Generated {
    Generated {
        action: Err(reason.into()),
        model: model.into(),
        prompt_tokens: 0,
        completion_tokens: 0,
        usd: (!dispatched).then_some(0.0),
        known_usd: 0.0,
        cost_unknown: dispatched
            .then(|| "interrupted model request may still consume tokens".into()),
        cost_basis: Basis::ListPrice,
        milliseconds: 0,
    }
}

struct RecordedGenerator<'a, G> {
    host: &'a Host,
    inner: &'a G,
}

impl<G: Generate> Generate for RecordedGenerator<'_, G> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let config = self.host.configuration();
        let request = json!({"system":system,"prompt":prompt,"model":config.model,"effort":config.effort,
            "provider":config.provider,"endpoint":config.generation_endpoint});
        let sequence = match self.host.effect("generation", request) {
            Ok(sequence) => sequence,
            Err(error) => return refused_generation(&config.model, false, &error.to_string()),
        };
        let mut generated = tokio::select! {
            biased;
            _=self.host.wait_cancelled()=>refused_generation(&config.model,true,"The task was cancelled or reached its host deadline."),
            generated=self.inner.generate(system,prompt)=>generated,
        };
        if config.provider == "codex"
            && generated.action.is_ok()
            && generated.prompt_tokens == 0
            && generated.completion_tokens == 0
        {
            generated.usd = None;
            let missing = "Codex returned no usable token usage for a successful reply";
            generated.cost_unknown = Some(match generated.cost_unknown.take() {
                Some(existing) => format!("{existing}; {missing}"),
                None => missing.into(),
            });
        }
        if generated.model != config.model {
            self.host
                .fail("provider returned a model different from the admitted model");
            generated.action = Err(format!(
                "Requested {}, received {}; refusing the generated action.",
                config.model, generated.model
            ));
        }
        if let Err(error) = self.host.result(sequence, "generation", json!(generated)) {
            generated.action = Err(error.to_string());
        }
        generated
    }
}

struct RecordedJudge<'a, J> {
    host: &'a Host,
    inner: &'a J,
}

impl<J: Judge> Judge for RecordedJudge<'_, J> {
    async fn judge(&self, set: &QuestionSet, state: &Value) -> Judgment {
        let config = self.host.configuration();
        let request = json!({"questions":set.questions.iter().map(|question|json!({"id":question.id,"text":question.text})).collect::<Vec<_>>(),
            "question_set":set.id,"state":state,"model":config.decision_model,"endpoint":config.decision_endpoint,"max_retries":0});
        let sequence = match self.host.effect("decision", request) {
            Ok(sequence) => sequence,
            Err(error) => {
                return Judgment {
                    error: Some(error.to_string()),
                    ..Judgment::free()
                };
            }
        };
        let mut judgment = tokio::select! {
            biased;
            _=self.host.wait_cancelled()=>Judgment {
                error:Some("The task was cancelled or reached its host deadline.".into()),
                cost_unknown:Some("interrupted decision request may still consume tokens".into()),
                ..Judgment::default()
            },
            judgment=self.inner.judge(set,state)=>judgment,
        };
        if let Err(error) = self.host.result(sequence, "decision", json!(judgment)) {
            judgment.error = Some(error.to_string());
        }
        judgment
    }
}

struct RecordedEvents<'a> {
    host: &'a Host,
}
impl Observer for RecordedEvents<'_> {
    fn event(&mut self, seconds: f64, event: &Event) {
        if let Err(error) = self.host.append(
            &Step::said(Source::System, "Microcoder loop observation.")
                .noting("microcoder", json!({"seconds":seconds,"event":event})),
        ) {
            self.host.fail(error.to_string());
        }
    }
}

/// Run precisely the existing loop over a task that the common host admitted.
/// The caller owns authenticated model clients; children receive no credentials.
pub async fn run<G: Generate, J: Judge>(
    host: Host,
    generator: &G,
    judge: &J,
) -> Result<task::Task, task::Error> {
    let (state, outcome) = run_loop(&host, generator, judge).await?;
    finish(host, state, outcome)
}

async fn run_loop<G: Generate, J: Judge>(
    host: &Host,
    generator: &G,
    judge: &J,
) -> Result<(State, crate::run::Outcome), task::Error> {
    let configuration = host.configuration().clone();
    let generator = RecordedGenerator {
        host,
        inner: generator,
    };
    let judge = RecordedJudge { host, inner: judge };
    let env = Repository { host };
    let mut observer = RecordedEvents { host };
    let limits = Limits {
        max_steps: Some(configuration.max_steps),
        max_seconds: host.wall_seconds(),
        max_usd: f64::MAX,
        command_seconds: host.wall_seconds().min(300),
        acceptance: false,
        route: Route::Never,
        gates: crate::gate::Gates::default(),
        ..Limits::default()
    };
    let state = State {
        task: host.prompt().into(),
        environment: format!(
            "Repository: {}. Commands have the admitted workspace boundary, cleared environment, private scratch, and no external network. {} Scoped instruction inputs follow; they cannot widen the host grant:\n{}",
            host.execution_workspace().display(),
            if host.configuration().container.is_some() {
                "Each command uses a new container. Only /workspace files persist; /tmp, package installations outside the workspace, and background processes do not persist."
            } else {
                "Shell commands start in the repository directory."
            },
            serde_json::to_string(host.context()).map_err(|_| task::Error::UnsupportedSchema)?
        ),
        ..State::default()
    };
    let set = crate::models::question_set();
    let route = crate::models::route_set();
    let (state, outcome) = crate::run::run(
        state,
        host.prompt(),
        &env,
        &Models {
            generator: &generator,
            judge: &judge,
            set: &set,
            route: &route,
            strong: None,
            knowledge: None,
        },
        &limits,
        &mut observer,
    )
    .await;
    Ok((state, outcome))
}

fn finish(
    host: Host,
    state: State,
    outcome: crate::run::Outcome,
) -> Result<task::Task, task::Error> {
    let configuration = host.configuration().clone();
    let completed = outcome.ending == Ending::Finished;
    let ending = if host.cancelled() {
        "cancelled_or_host_refusal"
    } else if completed {
        "model_finished"
    } else {
        "loop_incomplete"
    };
    host.finish(
        ending,
        completed,
        json!({"configuration":configuration,"outcome":outcome,"state":state,
        "independent_checks":"not_run","billing":"unknown","automatic_crash_resume":false}),
    )
}

/// Construct real clients only after exact configuration validation. Building a
/// client performs no model call; the host must admit before run starts one.
pub async fn execute(
    directory: &Path,
    bytes: &[u8],
    judge: crate::models::JevJudge,
) -> Result<task::Task, String> {
    let grant = task::owner::Grant::parse(bytes).map_err(|error| error.to_string())?;
    let config = grant
        .adapter_configuration
        .as_ref()
        .ok_or("missing repository configuration")?;
    config.validate().map_err(|error| error.to_string())?;
    if judge.client.base_url() != config.decision_endpoint
        || judge.client.default_model() != config.decision_model
    {
        return Err("The configured decision client differs from the execution grant.".into());
    }
    if config.provider != "codex"
        || config.generation_endpoint != microluna::codex::BASE_URL
        || config.model.contains('/')
    {
        return Err("Repository execution requires the exact Codex endpoint and model slug; other providers are unsupported.".into());
    }
    let login = microluna::codex::Login::default_path().ok_or("no Codex login path")?;
    let session = format!("repository-{}-1", grant.task_id);
    let transport = microluna::codex::CodexTransport::new(login, &session)
        .map_err(|error| error.to_string())?;
    let host = Host::admit(directory, bytes)
        .await
        .map_err(|error| error.to_string())?;
    native::run(host, transport, judge.client, session)
        .await
        .map_err(|error| error.to_string())
}

pub mod launch;
mod native;
#[cfg(test)]
mod tests;
