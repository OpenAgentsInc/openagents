//! Configuration and native resume use the local task owner's existing grants.

use super::*;
use route_contract::{digest_of, session::control as wire};
use std::io::Read;

const TRACE_BYTES: u64 = 64 * 1024 * 1024;

impl Local {
    pub(super) fn session_account(&self, provider: Provider) -> Option<String> {
        #[cfg(test)]
        if std::ptr::fn_addr_eq(self.identify, account::identify as account::Identify) {
            return None;
        }
        account::current(&self.store, provider, self.identify)
    }

    /// The supported coding choices for this exact project and login state.
    pub fn project_session_options(&self, dir: &Path) -> Result<wire::Offer, String> {
        let checkout = self.project(dir)?;
        let policy = self.policy(&checkout.name)?;
        // A new store needs its private account salt before choices can bind
        // the current login. This creates no task or provider conversation.
        drop(Store::open(&self.store).map_err(|error| error.to_string())?);
        self.session_offer(
            &policy,
            None,
            0,
            0,
            &checkout.top,
            &checkout.head,
            None,
            false,
            None,
        )
    }

    /// Read choices for a task without creating a new turn or provider process.
    pub fn session_options(&self, task: &str) -> Result<wire::Offer, String> {
        if super::super::issue_run::load(&self.store, task).is_some_and(|flow| !flow.finished) {
            return Err("Use default coding settings while Coder works on an issue.".into());
        }
        let record = record(&self.store, task).ok_or("This task is not on this computer.")?;
        let current = Store::open(&self.store)
            .and_then(|store| store.show(task))
            .map_err(|e| e.to_string())?;
        let (policy, _) = self.shaped(
            self.policy(&record.project)?,
            record.requested.as_deref().and_then(Provider::from_config),
            record.shape,
        )?;
        let source = self.native_session(&record, &current);
        let ended = matches!(current.status, Status::Finished | Status::Cancelled)
            && current.checks != super::super::Checks::Running
            && current
                .run
                .as_ref()
                .and_then(|run| run.result.as_ref())
                .is_some_and(|result| result.group_clear);
        let cleaned = if ended {
            Store::open(&self.store)
                .ok()
                .is_some_and(|store| super::super::owner::Owner::acquire(&store, task).is_ok())
        } else {
            false
        };
        let native = source
            .as_ref()
            .filter(|_| cleaned && record.archived.is_none());
        self.session_offer(
            &policy,
            Some(task),
            current.turn(),
            current.revision,
            Path::new(&record.worktree),
            &record.base,
            native,
            !current.intent.images.is_empty(),
            record
                .archived
                .as_ref()
                .map(|archived| format!("restore:{}", digest_of(archived).as_str())),
        )
    }

    fn session_offer(
        &self,
        policy: &Policy,
        task: Option<&str>,
        turn: usize,
        revision: u64,
        checkout: &Path,
        base: &str,
        native: Option<&wire::Native>,
        images: bool,
        restore: Option<String>,
    ) -> Result<wire::Offer, String> {
        let target = super::super::pending::target_identity(checkout)
            .or_else(|_| restore.ok_or(super::super::Error::UnsafePath))
            .map_err(|_| "This project folder changed. Choose it again.")?;
        let mut models = Vec::new();
        for route in session_routes(policy) {
            let config = policy.configuration(std::slice::from_ref(&route));
            let backend = describe_backend(
                &config.provider,
                &config.model,
                config.effort.clone(),
                &config.generation_endpoint,
            );
            let connected = (self.probe)(route.provider).is_connected();
            let can_image = !images || matches!(route.provider, Provider::Claude | Provider::Codex);
            let available = connected && can_image && route.refusal(policy.engine.access).is_none();
            models.push(wire::Model {
                selection:backend.selection,adapter:backend.adapter.clone(),
                efforts:coder_sdk::session::efforts(&backend.adapter).iter().map(|s|(*s).into()).collect(),
                available,
                image_followups:backend.native.image_followups,
                modes: if task.is_none() {vec![wire::Mode::New]} else if backend.adapter=="claude-sdk" {vec![wire::Mode::Continue,wire::Mode::New,wire::Mode::ContextTransfer]} else {vec![wire::Mode::Continue]},
                account:if backend.adapter=="claude-sdk" {self.session_account(route.provider)} else {None},
                reason:if available {None} else if !can_image {Some("Choose Claude Code or Codex for this task's images.".into())}
                    else {Some(format!("Install and sign in to {} on this computer, and enable it in Coder settings.",settings::provider_name(route.provider)))},
            });
        }
        let resume = native
            .filter(|native| {
                native.valid()
                    && native.host == LOCAL_HOST
                    && Some(native.task.as_str()) == task
                    && same_folder(Path::new(&native.checkout), checkout)
                    && native.target == target
                    && native.account.is_some()
                    && models.iter().any(|model| {
                        model.available
                            && model.adapter == native.adapter
                            && model.selection.provider == native.provider
                            && model.selection.model == native.model
                            && model.account == native.account
                    })
            })
            .map(wire::Native::reference);
        if let Some(resume) = &resume {
            for model in &mut models {
                if model.adapter == "claude-sdk"
                    && model.selection.provider == resume.provider
                    && model.selection.model == resume.model
                {
                    model.modes.push(wire::Mode::NativeResume);
                }
            }
        }
        let mut stable_policy = policy.clone();
        stable_policy.changed_at = 0;
        stable_policy.engine.model.clear();
        stable_policy.engine.routes.sort_by_key(|route| {
            format!(
                "{}:{}:{:?}:{:?}",
                route.provider.as_str(),
                route.model,
                route.effort,
                route.engine
            )
        });
        let mut offer = wire::Offer {
            schema: wire::SCHEMA.into(),
            id: digest_of(&()),
            task: task.map(str::to_owned),
            revision,
            turn,
            authority: digest_of(&stable_policy),
            checkout: checkout.display().to_string(),
            target,
            base: base.into(),
            models,
            resume,
        };
        offer.seal();
        if !offer.valid() {
            return Err("These coding choices could not be loaded.".into());
        }
        Ok(offer)
    }

    pub(super) fn native_session(
        &self,
        record: &Record,
        current: &super::super::Task,
    ) -> Option<wire::Native> {
        let run = current.run.as_ref()?;
        let file = Path::new(&run.admission.trace_file);
        if file.components().count() != 1 {
            return None;
        }
        let path = self.store.join(file);
        let mut bytes = Vec::new();
        super::super::private_open(&path, false, false)
            .ok()?
            .take(TRACE_BYTES + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() as u64 > TRACE_BYTES {
            return None;
        }
        let native = bytes
            .split(|byte| *byte == b'\n')
            .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
            .filter(|line| {
                line["record"] == "step"
                    && matches!(line["step"]["source"].as_str(), Some("System" | "system"))
            })
            .filter_map(|line| {
                serde_json::from_value::<wire::Native>(
                    line["step"]["extensions"][wire::NATIVE_NOTE].clone(),
                )
                .ok()
            })
            .last()?;
        (native.valid()
            && native.host == LOCAL_HOST
            && native.task == record.task
            && native.turn == current.turn()
            && Path::new(&native.checkout) == run.admission.workspace
            && native.adapter == "claude-sdk"
            && run
                .admission
                .grant
                .adapter_configuration
                .as_ref()
                .is_some_and(|config| {
                    config.routes().iter().any(|route| {
                        route.provider == native.provider
                            && route.model == native.model
                            && route.generation_endpoint == capacity::CLAUDE_SDK_ENDPOINT
                    })
                }))
        .then_some(native)
    }

    pub(super) fn select_session(
        &self,
        mut policy: Policy,
        offer: &wire::Offer,
        request: &wire::Request,
    ) -> Result<(Policy, wire::Start), String> {
        if !offer.accepts(request) {
            return Err("These coding choices changed. Check the model and try again.".into());
        }
        let model = offer
            .models
            .iter()
            .find(|model| {
                model.selection.provider == request.selection.provider
                    && model.selection.model == request.selection.model
                    && model.adapter == request.adapter
            })
            .ok_or("This model is unavailable.")?;
        let mut route = session_routes(&policy)
            .into_iter()
            .find(|route| {
                route.provider.as_str() == request.selection.provider
                    && route.model == request.selection.model
                    && {
                        let config = policy.configuration(std::slice::from_ref(route));
                        describe_backend(
                            &config.provider,
                            &config.model,
                            config.effort.clone(),
                            &config.generation_endpoint,
                        )
                        .adapter
                            == request.adapter
                    }
            })
            .ok_or("This model is not enabled on this computer.")?;
        route.effort = request.selection.effort.clone();
        policy.engine.model = route.model.clone();
        policy.engine.effort = route.effort.clone();
        policy.engine.routes = vec![route];
        policy.validate()?;
        Ok((
            policy,
            wire::Start {
                request: Some(request.clone()),
                selection: request.selection.clone(),
                account: model.account.clone(),
                task_model: None,
                context: None,
                native: None,
            },
        ))
    }
    pub(super) fn answer_session(
        &self,
        task: &str,
        text: &str,
        configuration: Option<&wire::Request>,
    ) -> Result<Record, String> {
        if text.trim().is_empty() || text.len() > 32 * 1024 {
            return Err("Write a message of at most 32 KiB to continue.".into());
        }
        let _held = super::super::retire::lock(&self.store, task)?;
        let mut record = record(&self.store, task)
            .ok_or("This task was not started on this computer from a chat.")?;
        let current = Store::open(&self.store)
            .and_then(|store| store.show(task))
            .map_err(|e| e.to_string())?;
        if !matches!(current.status, Status::Finished | Status::Cancelled) {
            return Err("Coder is still working on this task; wait for it to finish.".into());
        }
        if current.checks == super::super::Checks::Running {
            return Err("Coder is still checking this task. Wait for the checks to finish.".into());
        }
        if current
            .run
            .as_ref()
            .and_then(|run| run.result.as_ref())
            .is_some_and(|result| !result.group_clear)
        {
            return Err(
                "Coder has not confirmed that this session stopped. Close it before continuing."
                    .into(),
            );
        }
        let native = self.native_session(&record, &current);
        let requested = record.requested.as_deref().and_then(Provider::from_config);
        let (policy, asked) =
            self.shaped(self.policy(&record.project)?, requested, record.shape)?;
        // An explicitly chosen model remains the next turn's choice until changed.
        let remembered = record
            .turns
            .last()
            .and_then(|turn| turn.session.as_ref())
            .and_then(|session| session.requested.as_ref())
            .filter(|selection| !selection.model.is_empty());
        let fresh;
        let request = if let Some(request) = configuration {
            Some(request)
        } else if let Some(selection) = remembered {
            let offer = self.session_options(task)?;
            fresh = wire::Request {
                adapter: record
                    .turns
                    .last()
                    .and_then(|turn| turn.session.as_ref())
                    .map(|session| session.resolved.adapter.clone())
                    .unwrap_or_default(),
                schema: wire::SCHEMA.into(),
                offer: offer.id.clone(),
                checkout: offer.checkout.clone(),
                target: offer.target.clone(),
                selection: selection.clone(),
                mode: wire::Mode::Continue,
                resume: None,
            };
            Some(&fresh)
        } else {
            None
        };
        let (policy, mut execution) = if let Some(request) = request {
            let offer = self.session_options(task)?;
            let (policy, mut start) = self.select_session(policy, &offer, request)?;
            if request.mode == wire::Mode::NativeResume {
                let source = native
                    .clone()
                    .filter(|source| Some(source.id()) == request.resume)
                    .ok_or("This saved session is unavailable. Start a new session instead.")?;
                start.native = Some(source);
            }
            (policy, Some(start))
        } else {
            (policy, None)
        };
        let (order, runner) = self.choose_asking(&policy, asked)?;
        let actual = policy.configuration(&order);
        if actual.generation_endpoint == capacity::CLAUDE_SDK_ENDPOINT
            && execution.as_ref().is_none_or(|start| !start.fresh())
        {
            let account = self.session_account(order[0].provider);
            if native
                .as_ref()
                .is_some_and(|native| native.account.is_some() && native.account != account)
            {
                return Err(
                    "Your Claude Code login changed. Start a new session to continue.".into(),
                );
            }
        }
        if let Some(execution) = &mut execution {
            execution.task_model = current.intent.configuration.model.clone();
        }
        if let Some(execution) = &mut execution
            && execution.mode() == wire::Mode::ContextTransfer
        {
            let summary = match record.ends.get(&current.turn()) {
                Some(CoderEvent::Result(result)) => result.summary.as_str(),
                _ => "",
            };
            execution.context = Some(wire::Context {
                task: task.into(),
                turn: current.turn(),
                text: format!(
                    "Original task:\n{}\n\nLatest message:\n{}\n\nLatest result:\n{}",
                    snippet(&current.intent.prompt, 4 * 1024),
                    snippet(&current.effective_prompt(), 4 * 1024),
                    snippet(summary, 6 * 1024 - 256)
                ),
            });
        }
        let prompt = text.trim().to_owned();
        if !request.is_some_and(|request| request.mode == wire::Mode::NativeResume) {
            super::super::retire::restore(&self.store, &mut record)?;
        }
        let command = Command {
            schema: COMMAND_SCHEMA.into(),
            command_id: identity(&format!("continue:{task}:{}:{}", current.revision, nonce())),
            task_id: task.into(),
            expected_revision: Some(current.revision),
            action: Action::Continue { prompt },
        };
        self.apply(&command)?;
        let next = Store::open(&self.store)
            .and_then(|store| store.show(task))
            .map_err(|e| e.to_string())?;
        if let Some(request) = request {
            record.requested = Some(request.selection.provider.clone());
        }
        self.launch(&policy, &order, runner, &next, &mut record, execution)?;
        Ok(record)
    }
}

fn snippet(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn same_folder(a: &Path, b: &Path) -> bool {
    a.canonicalize()
        .ok()
        .zip(b.canonicalize().ok())
        .is_some_and(|(a, b)| a == b)
}

/// The local owner can explicitly choose live approvals for an admitted Claude
/// model. This adds no provider, model, filesystem right, or automatic fallback.
pub(super) fn session_routes(policy: &Policy) -> Vec<Route> {
    let mut routes = policy.routes();
    if policy.engine.access == super::super::adapter::Access::Full {
        for mut route in policy
            .routes()
            .into_iter()
            .filter(|route| route.provider == Provider::Claude)
        {
            route.engine = Some(autostart::RouteEngine::Sdk);
            if !routes.contains(&route) {
                routes.push(route);
            }
        }
    }
    routes
}
