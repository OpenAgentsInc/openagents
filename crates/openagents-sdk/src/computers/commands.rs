use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use tokio::time::Instant;

#[derive(Clone, Serialize, Deserialize)]
pub struct OutputReader {
    v: String,
    command: CommandRef,
    stdout: u64,
    stderr: u64,
    ended: bool,
}
pub struct Output {
    pub command: Command,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_gap: bool,
    pub stderr_gap: bool,
    pub terminal: bool,
    pub has_more: bool,
}
impl OutputReader {
    pub fn command(&self) -> &CommandRef {
        &self.command
    }
    pub async fn next_page(
        &mut self,
        computers: &Computers<'_>,
        cancel: &CancellationToken,
    ) -> Result<Option<Output>, Error> {
        computers.check(&self.command.computer)?;
        identity(&self.command.id)?;
        if self.v != "openagents.computers.output-reader.v1" {
            return Err(Error::InvalidInput);
        }
        if self.ended {
            return Ok(None);
        }
        let plan = generated::AttachComputerCommandParams {
            id: self.command.computer.id.clone(),
            command: self.command.id.clone(),
            stdout: Some(self.stdout),
            stderr: Some(self.stderr),
            max_bytes: Some(65536),
            wait_ms: Some(1000),
        }
        .into_request()
        .map_err(|_| Error::InvalidInput)?;
        let result: generated::AppCommandOutput = computers.json(plan, cancel).await?;
        if result.encoding != "base64" {
            return Err(Error::InvalidResponse);
        }
        let stdout = STANDARD
            .decode(&result.stdout)
            .map_err(|_| Error::InvalidResponse)?;
        let stderr = STANDARD
            .decode(&result.stderr)
            .map_err(|_| Error::InvalidResponse)?;
        if stdout.len() > 65536 || stderr.len() > 65536 {
            return Err(Error::TooLarge);
        }
        let advance = |old: u64, new: u64, len: usize, gap: bool| {
            let Some(delta) = new.checked_sub(old) else {
                return false;
            };
            if gap {
                delta >= len as u64
            } else {
                delta == len as u64
            }
        };
        if !advance(
            self.stdout,
            result.next_cursor.stdout,
            stdout.len(),
            result.gaps.stdout,
        ) || !advance(
            self.stderr,
            result.next_cursor.stderr,
            stderr.len(),
            result.gaps.stderr,
        ) || result.has_more
            && result.next_cursor.stdout == self.stdout
            && result.next_cursor.stderr == self.stderr
        {
            return Err(Error::InvalidResponse);
        }
        let command = computers.command(
            result.command,
            &self.command.computer,
            Some(&self.command.id),
        )?;
        self.stdout = result.next_cursor.stdout;
        self.stderr = result.next_cursor.stderr;
        self.ended = result.terminal && !result.has_more;
        Ok(Some(Output {
            command,
            stdout,
            stderr,
            stdout_gap: result.gaps.stdout,
            stderr_gap: result.gaps.stderr,
            terminal: result.terminal,
            has_more: result.has_more,
        }))
    }
}
impl Computers<'_> {
    pub fn command_request(
        &self,
        computer: &ComputerRef,
        mut options: CommandOptions,
    ) -> Result<CommandRequest, Error> {
        self.check(computer)?;
        if options.command.is_empty() {
            return Err(Error::InvalidInput);
        }
        let key = request_key(&options.idempotency_key)?;
        options.idempotency_key = Some(key.as_str().into());
        Ok(CommandRequest {
            v: "openagents.computers.command-request.v1".into(),
            computer: computer.clone(),
            key,
            body: options,
        })
    }
    pub async fn execute(
        &self,
        request: &CommandRequest,
        cancel: &CancellationToken,
    ) -> Result<Command, Error> {
        self.check(&request.computer)?;
        if request.v != "openagents.computers.command-request.v1"
            || request.body.idempotency_key.as_deref() != Some(request.key.as_str())
        {
            return Err(Error::InvalidInput);
        }
        let plan = generated::CreateComputerCommandParams {
            id: request.computer.id.clone(),
            idempotency_key: Some(request.key.as_str().into()),
            body: request.body.clone(),
        }
        .into_request()
        .map_err(|_| Error::InvalidInput)?;
        self.command(self.json(plan, cancel).await?, &request.computer, None)
            .map_err(|cause| Error::Uncertain {
                operation: "createComputerCommand",
                key: Some(request.key.as_str().into()),
                cause: Box::new(cause),
            })
    }
    pub async fn get_command(
        &self,
        reference: &CommandRef,
        cancel: &CancellationToken,
    ) -> Result<Command, Error> {
        self.check(&reference.computer)?;
        identity(&reference.id)?;
        let plan = generated::GetComputerCommandParams {
            id: reference.computer.id.clone(),
            command: reference.id.clone(),
        }
        .into_request()
        .map_err(|_| Error::InvalidInput)?;
        self.command(
            self.json(plan, cancel).await?,
            &reference.computer,
            Some(&reference.id),
        )
    }
    pub async fn cancel_command(
        &self,
        reference: &CommandRef,
        cancel: &CancellationToken,
    ) -> Result<Command, Error> {
        self.check(&reference.computer)?;
        identity(&reference.id)?;
        let plan = generated::CancelComputerCommandParams {
            id: reference.computer.id.clone(),
            command: reference.id.clone(),
        }
        .into_request()
        .map_err(|_| Error::InvalidInput)?;
        self.command(
            self.json(plan, cancel).await?,
            &reference.computer,
            Some(&reference.id),
        )
        .map_err(|cause| Error::Uncertain {
            operation: "cancelComputerCommand",
            key: None,
            cause: Box::new(cause),
        })
    }
    pub fn output(&self, reference: &CommandRef) -> Result<OutputReader, Error> {
        self.check(&reference.computer)?;
        identity(&reference.id)?;
        Ok(OutputReader {
            v: "openagents.computers.output-reader.v1".into(),
            command: reference.clone(),
            stdout: 0,
            stderr: 0,
            ended: false,
        })
    }
    pub async fn wait_command(
        &self,
        reference: &CommandRef,
        timeout: Duration,
        cancel: &CancellationToken,
    ) -> Result<Command, Error> {
        if timeout.is_zero() {
            return Err(Error::WaitTimedOut);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(Error::InvalidInput)?;
        let polling = async {
            loop {
                let command = self.get_command(reference, cancel).await?;
                if command.outcome().is_some() {
                    return Ok(command);
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        };
        tokio::select! {biased;_=cancel.cancelled()=>Err(Error::Cancelled),_=tokio::time::sleep_until(deadline)=>Err(Error::WaitTimedOut),value=polling=>value}
    }
}
opaque_debug!(OutputReader, Output);
