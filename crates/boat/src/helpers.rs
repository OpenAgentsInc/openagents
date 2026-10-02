use std::{collections::VecDeque, future::Future, time::Duration};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use tokio::{sync::watch, time::Instant};

use crate::{Client, Error, Result, models::*};

/// Shared cancellation for requests and polling. Cancellation does not stop remote work.
#[derive(Clone, Debug)]
pub struct Cancellation(watch::Sender<bool>);

impl Default for Cancellation {
    fn default() -> Self {
        Self(watch::channel(false).0)
    }
}

impl Cancellation {
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }

    pub async fn cancelled(&self) {
        let mut receiver = self.0.subscribe();
        let _ = receiver.wait_for(|value| *value).await;
    }

    /// Bound any SDK call or download with cancellation and an absolute deadline.
    pub async fn run<T>(
        &self,
        deadline: Instant,
        work: impl Future<Output = Result<T>>,
    ) -> Result<T> {
        tokio::select! {
            biased;
            _ = self.cancelled() => Err(Error::Cancelled),
            _ = tokio::time::sleep_until(deadline) => Err(Error::Deadline),
            result = work => result,
        }
    }
}

#[derive(Clone, Debug)]
pub struct WaitOptions {
    pub timeout: Duration,
    pub interval: Duration,
    pub cancellation: Cancellation,
}

impl Default for WaitOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300),
            interval: Duration::from_secs(2),
            cancellation: Cancellation::default(),
        }
    }
}

impl WaitOptions {
    fn deadline(&self) -> Result<Instant> {
        if self.timeout.is_zero() || self.interval.is_zero() {
            return Err(Error::Configuration(
                "Polling limits must be greater than zero.",
            ));
        }
        Instant::now()
            .checked_add(self.timeout)
            .ok_or(Error::Configuration("The polling deadline is too large."))
    }

    async fn pause(&self) {
        tokio::time::sleep(self.interval).await;
    }
}

impl Client {
    pub async fn wait_until_ready(
        &self,
        sandbox_id: &str,
        options: &WaitOptions,
    ) -> Result<Sandbox> {
        options
            .cancellation
            .run(options.deadline()?, async {
                loop {
                    let info = self
                        .get(&GetParams {
                            sandbox_id: sandbox_id.into(),
                            ..Default::default()
                        })
                        .await?;
                    match info.sandbox.state.as_str() {
                        "ready" | "idle" | "running" => return Ok(info.sandbox),
                        "archived" | "archiving" | "error" | "cancelled" => {
                            return Err(Error::TerminalState);
                        }
                        _ => options.pause().await,
                    }
                }
            })
            .await
    }

    pub async fn wait_for_prompt(
        &self,
        sandbox_id: &str,
        prompt_id: &str,
        options: &WaitOptions,
    ) -> Result<PromptRun> {
        options
            .cancellation
            .run(options.deadline()?, async {
                loop {
                    let run = self
                        .prompt_run_status(&PromptRunStatusParams {
                            sandbox_id: sandbox_id.into(),
                            prompt_id: prompt_id.into(),
                            ..Default::default()
                        })
                        .await?
                        .prompt_run;
                    if run.done || matches!(run.status.as_str(), "finished" | "failed") {
                        return Ok(run);
                    }
                    options.pause().await;
                }
            })
            .await
    }

    pub async fn wait_for_command(
        &self,
        params: &CommandStatusParams,
        options: &WaitOptions,
    ) -> Result<CommandStatusResponse> {
        options
            .cancellation
            .run(options.deadline()?, async {
                loop {
                    let status = self.command_status(params).await?;
                    if status.status == "lost" {
                        return Err(Error::TerminalState);
                    }
                    if !status.running {
                        return Ok(status);
                    }
                    options.pause().await;
                }
            })
            .await
    }

    pub async fn wait_for_deletion(
        &self,
        operation_id: &str,
        options: &WaitOptions,
    ) -> Result<DeletionOperation> {
        options
            .cancellation
            .run(options.deadline()?, async {
                loop {
                    let operation = self
                        .get_deletion_operation(&GetDeletionOperationParams {
                            operation_id: operation_id.into(),
                            ..Default::default()
                        })
                        .await?
                        .operation;
                    match operation.status.as_str() {
                        "completed" => return Ok(operation),
                        "blocked" => {
                            return Err(Error::DeletionBlocked(std::boxed::Box::new(operation)));
                        }
                        "failed" => return Err(Error::TerminalState),
                        _ => options.pause().await,
                    }
                }
            })
            .await
    }

    pub async fn wait_for_desktop(
        &self,
        params: &DesktopParams,
        options: &WaitOptions,
    ) -> Result<DesktopResponse> {
        options
            .cancellation
            .run(options.deadline()?, async {
                loop {
                    let desktop = self.desktop(params).await?;
                    if desktop.provisioning != Some(true)
                        && desktop.desktop_url.as_ref().is_some_and(|u| !u.is_empty())
                    {
                        return Ok(desktop);
                    }
                    options.pause().await;
                }
            })
            .await
    }

    pub async fn read_text(&self, sandbox_id: &str, path: &str) -> Result<String> {
        Ok(self
            .read_file(&ReadFileParams {
                sandbox_id: sandbox_id.into(),
                path: path.into(),
                encoding: Some("utf8".into()),
                ..Default::default()
            })
            .await?
            .content)
    }

    pub async fn write_text(
        &self,
        sandbox_id: &str,
        path: &str,
        text: &str,
    ) -> Result<FileWriteResponse> {
        self.write_file(&WriteFileParams {
            sandbox_id: sandbox_id.into(),
            body: FileWriteRequest {
                path: path.into(),
                content: text.into(),
                encoding: Some("utf8".into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
    }

    pub async fn read_bytes(&self, sandbox_id: &str, path: &str) -> Result<Vec<u8>> {
        let file = self
            .read_file(&ReadFileParams {
                sandbox_id: sandbox_id.into(),
                path: path.into(),
                encoding: Some("base64".into()),
                ..Default::default()
            })
            .await?;
        base64::engine::general_purpose::STANDARD
            .decode(file.content)
            .map_err(|_| Error::Decode)
    }

    pub async fn write_bytes(
        &self,
        sandbox_id: &str,
        path: &str,
        bytes: &[u8],
    ) -> Result<FileWriteResponse> {
        self.write_file(&WriteFileParams {
            sandbox_id: sandbox_id.into(),
            body: FileWriteRequest {
                path: path.into(),
                content: base64::engine::general_purpose::STANDARD.encode(bytes),
                encoding: Some("base64".into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
    }

    /// Poll events from the supplied cursor, or from the beginning when omitted.
    pub fn stream_events(
        &self,
        mut params: EventsParams,
        options: WaitOptions,
    ) -> Result<EventStream> {
        params.sort = Some("asc".into());
        Ok(EventStream {
            client: self.clone(),
            params,
            deadline: options.deadline()?,
            options,
            pending: VecDeque::new(),
        })
    }
}

/// A resumable event reader. Persist `cursor()` after processing each event.
pub struct EventStream {
    client: Client,
    params: EventsParams,
    options: WaitOptions,
    deadline: Instant,
    pending: VecDeque<SandboxEvent>,
}

impl EventStream {
    pub fn cursor(&self) -> Option<&str> {
        self.params.cursor.as_deref()
    }

    pub async fn next(&mut self) -> Result<SandboxEvent> {
        let cancellation = self.options.cancellation.clone();
        cancellation
            .run(self.deadline, async {
                loop {
                    if let Some(event) = self.pending.pop_front() {
                        let id = event.id.as_deref().ok_or(Error::InvalidCursor)?;
                        let timestamp = event.timestamp.ok_or(Error::InvalidCursor)?;
                        let cursor = URL_SAFE_NO_PAD.encode(format!("{timestamp}:{id}"));
                        if self.params.cursor.as_deref() == Some(&cursor) {
                            return Err(Error::InvalidCursor);
                        }
                        self.params.cursor = Some(cursor);
                        return Ok(event);
                    }
                    let page = self.client.events(&self.params).await?;
                    self.pending.extend(page.events);
                    if self.pending.is_empty() {
                        self.options.pause().await;
                    }
                }
            })
            .await
    }
}
