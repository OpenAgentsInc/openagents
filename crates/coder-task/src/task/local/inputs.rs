//! Image-bearing follow-ups use one durable input identity across retries.

use super::super::{Error, media, resources};
use super::*;
use route_contract::{resources as wire, session::control};
use std::io::Read as _;

#[derive(Clone, Serialize, Deserialize)]
struct Operation {
    schema: String,
    input: wire::Input,
    prompt: String,
    original: Option<control::Request>,
    offer: control::Offer,
    request: control::Request,
    start: control::Start,
    command: Command,
    target: String,
    launched: bool,
}
impl Local {
    pub fn answer_input(
        &self,
        task: &str,
        message: &str,
        text: &str,
        images: &[media::wire::Upload],
        configuration: Option<&control::Request>,
    ) -> Result<(wire::Input, wire::Receipt), String> {
        let prompt = text.trim();
        if message.is_empty() || message.len() > 128 || message.chars().any(char::is_control) {
            return Err("This message couldn't be sent. Send it again as a new message.".into());
        }
        if images.len() > wire::MAX_INPUTS
            || images
                .iter()
                .any(|image| image.bytes.len() > wire::MAX_BYTES)
            || images.iter().map(|image| image.bytes.len()).sum::<usize>() > wire::MAX_INPUT_BYTES
        {
            return Err("Attach up to four PNG or JPEG images, totaling 16 MiB or less.".into());
        }
        if prompt.is_empty()
            || prompt.len() > 32 * 1024
            || images.is_empty()
            || images.len() > wire::MAX_INPUTS
        {
            return Err("Write a message and attach up to four PNG or JPEG images.".into());
        }
        let _held = super::super::retire::lock(&self.store, task)?;
        let mut record = record(&self.store, task).ok_or("This task is not on this computer.")?;
        let current = Store::open(&self.store)
            .and_then(|store| store.show(task))
            .map_err(|error| error.to_string())?;
        let known = current.follow_ups.iter().find_map(|follow| {
            follow
                .input
                .as_ref()
                .filter(|input| input.message == message)
                .map(|input| (follow, input))
        });
        let turn = known.map_or_else(
            || current.turn() + usize::from(current.run.is_some()),
            |(_, input)| input.turn,
        );
        let references=images.iter().map(|image|resources::image_reference(task,message,&image.reference,&image.bytes)).collect::<Result<Vec<_>,_>>().map_err(|_|"An image couldn't be read. Choose valid PNG or JPEG images up to 4096 × 4096 pixels.")?;
        let mut input = wire::Input {
            schema: wire::SCHEMA.into(),
            id: route_contract::digest_of(&()),
            message: message.into(),
            turn,
            owner: wire::Owner {
                host: LOCAL_HOST.into(),
                task: task.into(),
            },
            text: route_contract::Digest::of_bytes(prompt.as_bytes()),
            choice: configuration.map(route_contract::digest_of),
            resources: references,
        };
        input.seal();
        if !input.valid_for(task, prompt) {
            return Err(
                "These images exceed the message's size limit. Remove an image and try again."
                    .into(),
            );
        }
        if let Some((follow, accepted)) = known {
            if *accepted != input || follow.prompt != prompt {
                return Err(
                    "This message changed after it was sent. Send it as a new message.".into(),
                );
            }
            if current.follow_ups.last() == Some(follow)
                && current.status == Status::Cancelled
                && current.run.is_none()
            {
                return Err(
                    "This message was stopped before it ran. Send it again as a new message."
                        .into(),
                );
            }
            if current.follow_ups.last() != Some(follow) || current.status != Status::Queued {
                return Ok((input.clone(), receipt(&input, follow.revision)));
            }
        }
        let directory = resources::directory(&self.store, task)
            .map_err(|_| "These attachments couldn't be saved. Try again.")?;
        let path = directory.join("input.json");
        let saved=load(&path).map_err(|_|"The previous image message couldn't be read. Start a new chat to send these images.")?;
        let mut operation = if let Some(saved) =
            saved.filter(|saved| saved.input.message == message)
        {
            if saved.input != input
                || saved.prompt != prompt
                || saved.original.as_ref() != configuration
                || saved.schema != wire::SCHEMA
                || !saved.offer.accepts(&saved.request)
                || saved.start.request.as_ref() != Some(&saved.request)
                || saved.request.adapter != "claude-sdk"
                || saved.start.selection != saved.request.selection
                || saved.command.schema != COMMAND_SCHEMA
                || saved.command.task_id != task
                || saved.command.command_id != identity(&format!("input:{task}:{message}"))
                || saved.command.action
                    != (Action::ContinueInput {
                        prompt: prompt.into(),
                        input: input.clone(),
                    })
                || saved.command.expected_revision.is_none()
                || !saved.start.valid_for(
                    &saved.request.selection.provider,
                    &saved.request.selection.model,
                    saved.request.selection.effort.as_deref(),
                )
            {
                return Err(
                    "This message changed after it was sent. Send it as a new message.".into(),
                );
            }
            if saved.launched {
                let follow = known
                    .ok_or("This message is unavailable. Send it as a new message.")?
                    .0;
                return Ok((input.clone(), receipt(&input, follow.revision)));
            }
            saved
        } else {
            if known.is_some() {
                return Err("This message couldn't start. Send a new message to try again.".into());
            }
            if !current.ended() {
                return Err(
                    "Coder is still working. Wait for it to finish before sending these images."
                        .into(),
                );
            }
            let offer = self.session_options(task)?;
            let request = if let Some(request) = configuration {
                request.clone()
            } else {
                let selected = record
                    .turns
                    .last()
                    .and_then(|turn| turn.session.as_ref())
                    .filter(|session| session.resolved.adapter == "claude-sdk")
                    .ok_or("Choose Claude Code with live approvals to send images.")?;
                control::Request {
                    schema: control::SCHEMA.into(),
                    offer: offer.id.clone(),
                    adapter: "claude-sdk".into(),
                    checkout: offer.checkout.clone(),
                    target: offer.target.clone(),
                    selection: selected.resolved.selection.clone(),
                    mode: control::Mode::Continue,
                    resume: None,
                }
            };
            if request.adapter != "claude-sdk" || !offer.accepts(&request) {
                return Err("Choose Claude Code with live approvals to send images.".into());
            }
            let (policy, _) = self.shaped(
                self.policy(&record.project)?,
                record.requested.as_deref().and_then(Provider::from_config),
                record.shape,
            )?;
            let (_, mut start) = self.select_session(policy, &offer, &request)?;
            if request.mode == control::Mode::NativeResume {
                start.native = Some(
                    self.native_session(&record, &current)
                        .filter(|native| Some(native.id()) == request.resume)
                        .ok_or(
                            "This saved session is unavailable. Choose a new session instead.",
                        )?,
                );
            }
            if request.mode == control::Mode::ContextTransfer {
                let summary = match record.ends.get(&current.turn()) {
                    Some(CoderEvent::Result(result)) => result.summary.as_str(),
                    _ => "",
                };
                start.context = Some(control::Context {
                    task: task.into(),
                    turn: current.turn(),
                    text: format!(
                        "Original task:\n{}\n\nLatest message:\n{}\n\nLatest result:\n{}",
                        clip(&current.intent.prompt, 4 * 1024),
                        clip(current.effective_prompt(), 4 * 1024),
                        clip(summary, 6 * 1024 - 256)
                    ),
                });
            }
            start.task_model = current.intent.configuration.model.clone();
            if request.mode != control::Mode::NativeResume {
                super::super::retire::restore(&self.store, &mut record)?;
            }
            let target = super::super::pending::target_identity(Path::new(&record.worktree))
                .map_err(|_| "This folder changed. Choose the coding settings again.")?;
            if record.workspace_target.as_ref() != Some(&target) {
                return Err("This folder changed. Choose the coding settings again.".into());
            }
            storage_budget(&self.store, task, images).map_err(
                |_| "This chat has too many attached images. Start a new chat to add more.",
            )?;
            let command = Command {
                schema: COMMAND_SCHEMA.into(),
                command_id: identity(&format!("input:{task}:{message}")),
                task_id: task.into(),
                expected_revision: Some(current.revision),
                action: Action::ContinueInput {
                    prompt: prompt.into(),
                    input: input.clone(),
                },
            };
            let operation = Operation {
                schema: wire::SCHEMA.into(),
                input: input.clone(),
                prompt: prompt.into(),
                original: configuration.cloned(),
                offer,
                request,
                start,
                command,
                target,
                launched: false,
            };
            save_operation(&directory, &operation)?;
            operation
        };
        let target = super::super::pending::target_identity(Path::new(&record.worktree))
            .map_err(|_| "This folder changed. Start a new chat to send these images.")?;
        if operation.target != target || record.workspace_target.as_ref() != Some(&target) {
            return Err("This folder changed. Start a new chat to send these images.".into());
        }
        let (policy, asked) = self.shaped(
            self.policy(&record.project)?,
            record.requested.as_deref().and_then(Provider::from_config),
            record.shape,
        )?;
        let (policy, _) = self.select_session(policy, &operation.offer, &operation.request)?;
        let (order, runner) = self.choose_asking(&policy, asked)?;
        if operation.start.account != self.session_account(order[0].provider) {
            return Err("Your coding login changed. Send these images as a new message.".into());
        }
        for image in images {
            media::save(&self.store, task, &image.reference, &image.bytes)
                .map_err(|_| "An image couldn't be saved. Try again.")?;
        }
        self.apply(&operation.command)?;
        let next = Store::open(&self.store)
            .and_then(|store| store.show(task))
            .map_err(|error| error.to_string())?;
        if next.status != Status::Queued
            || next
                .follow_ups
                .last()
                .and_then(|follow| follow.input.as_ref())
                != Some(&input)
        {
            return Err(
                "This chat changed before the images could be sent. Send them as a new message."
                    .into(),
            );
        }
        record.requested = Some(operation.request.selection.provider.clone());
        record.turns.retain(|turn| turn.turn != next.turn());
        self.launch(
            &policy,
            &order,
            runner,
            &next,
            &mut record,
            Some(operation.start.clone()),
        )?;
        operation.launched = true;
        save_operation(&directory, &operation)?;
        Ok((input.clone(), receipt(&input, next.revision)))
    }
}
fn receipt(input: &wire::Input, revision: u64) -> wire::Receipt {
    wire::Receipt {
        schema: wire::SCHEMA.into(),
        input: input.id.clone(),
        task: input.owner.task.clone(),
        turn: input.turn,
        revision,
    }
}
fn save_operation(directory: &Path, operation: &Operation) -> Result<(), String> {
    let bytes = serde_json::to_vec(operation).map_err(|_| "This message couldn't be saved.")?;
    super::super::replace_file(directory, "input.json", &bytes)
        .map_err(|_| "This message couldn't be saved. Try again.".into())
}
fn load(path: &Path) -> Result<Option<Operation>, Error> {
    if !super::super::regular_or_absent(path)? {
        return Ok(None);
    }
    let mut bytes = vec![];
    super::super::private_open(path, false, false)?
        .take(512 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 512 * 1024 {
        return Err(Error::LimitExceeded);
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| Error::UnsupportedSchema)
}
fn clip(value: &str, limit: usize) -> &str {
    let mut end = limit.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}
fn storage_budget(store: &Path, task: &str, images: &[media::wire::Upload]) -> Result<(), Error> {
    let root = store.join(media::MEDIA_DIR).join(task);
    let mut total = 0usize;
    let mut known = std::collections::BTreeSet::new();
    if root.exists() {
        super::super::verify_directory(&root)?;
        for (count, entry) in std::fs::read_dir(&root)?.enumerate() {
            if count >= 1024 {
                return Err(Error::LimitExceeded);
            }
            let entry = entry?;
            let path = entry.path();
            super::super::regular_or_absent(&path)?;
            total = total
                .checked_add(entry.metadata()?.len() as usize)
                .ok_or(Error::LimitExceeded)?;
            known.insert(entry.file_name().to_string_lossy().into_owned());
        }
    }
    for image in images {
        let name = image.reference.digest.trim_start_matches("sha256:");
        if known.insert(name.into()) {
            total = total
                .checked_add(image.bytes.len())
                .ok_or(Error::LimitExceeded)?;
        }
    }
    if total > wire::MAX_TASK_BYTES {
        return Err(Error::LimitExceeded);
    }
    Ok(())
}
