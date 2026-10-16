use super::*;
use tokio::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitFor {
    Awake,
    Stopped,
    Deleted,
}

impl Computers<'_> {
    /// Create and retain this request before sending it. Serialization is
    /// explicit and contains private settings, but never a session credential.
    pub fn create_request(&self, mut options: CreateOptions) -> Result<CreateRequest, Error> {
        self.client.check_session(self.session)?;
        let key = request_key(&options.idempotency_key)?;
        options.idempotency_key = Some(key.as_str().into());
        Ok(CreateRequest {
            v: "openagents.computers.create-request.v1".into(),
            account: self.session.account().id.clone(),
            deployment: self.client.deployment.clone(),
            key,
            body: options,
        })
    }

    pub async fn create(
        &self,
        request: &CreateRequest,
        cancel: &CancellationToken,
    ) -> Result<Computer, Error> {
        self.client.check_session(self.session)?;
        if request.account != self.session.account().id {
            return Err(crate::Error::WrongAccount.into());
        }
        if request.deployment != self.client.deployment {
            return Err(crate::Error::WrongDeployment.into());
        }
        if request.v != "openagents.computers.create-request.v1"
            || request.body.idempotency_key.as_deref() != Some(request.key.as_str())
        {
            return Err(Error::InvalidInput);
        }
        let plan = generated::CreateComputerParams {
            idempotency_key: Some(request.key.as_str().into()),
            body: Some(request.body.clone()),
        }
        .into_request()
        .map_err(|_| Error::InvalidInput)?;
        let data = self.json(plan, cancel).await?;
        self.computer(data, None).map_err(|cause| Error::Uncertain {
            operation: "createComputer",
            key: Some(request.key.as_str().into()),
            cause: Box::new(cause),
        })
    }

    pub async fn get(
        &self,
        reference: &ComputerRef,
        cancel: &CancellationToken,
    ) -> Result<Computer, Error> {
        self.check(reference)?;
        let plan = generated::GetComputerParams {
            id: reference.id.clone(),
        }
        .into_request()
        .map_err(|_| Error::InvalidInput)?;
        self.computer(self.json(plan, cancel).await?, Some(&reference.id))
    }

    pub async fn list(
        &self,
        limit: u16,
        after: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<ComputerPage, Error> {
        if limit == 0 || limit > 200 || after.is_some_and(|s| s.len() > 4096) {
            return Err(Error::InvalidInput);
        }
        let plan = generated::ListComputersParams {
            kind: Some("cloud".into()),
            limit: Some(u64::from(limit)),
            after: after.map(str::to_owned),
        }
        .into_request()
        .map_err(|_| Error::InvalidInput)?;
        let result: generated::AppComputerList = self.json(plan, cancel).await?;
        if result.computers.len() > usize::from(limit)
            || result
                .next_cursor
                .as_deref()
                .is_some_and(|s| Some(s) == after || s.is_empty() || s.len() > 4096)
        {
            return Err(Error::InvalidResponse);
        }
        let computers = result
            .computers
            .into_iter()
            .map(|row| match row {
                generated::AppComputerListComputersItem::Variant0(row) => self.computer(row, None),
                _ => Err(Error::InvalidResponse),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ComputerPage {
            computers,
            next_cursor: result.next_cursor,
        })
    }

    pub async fn capabilities(&self, cancel: &CancellationToken) -> Result<Capabilities, Error> {
        self.json(
            generated::GetComputerCapabilitiesParams {}
                .into_request()
                .map_err(|_| Error::InvalidInput)?,
            cancel,
        )
        .await
    }

    /// An explicit intent; an uncertain stop is observed, never automatically replayed.
    pub async fn stop(
        &self,
        reference: &ComputerRef,
        cancel: &CancellationToken,
    ) -> Result<Computer, Error> {
        self.check(reference)?;
        self.computer(
            self.json(
                generated::StopComputerParams {
                    id: reference.id.clone(),
                }
                .into_request()
                .map_err(|_| Error::InvalidInput)?,
                cancel,
            )
            .await?,
            Some(&reference.id),
        )
        .map_err(|cause| Error::Uncertain {
            operation: "stopComputer",
            key: None,
            cause: Box::new(cause),
        })
    }
    pub async fn wake(
        &self,
        reference: &ComputerRef,
        cancel: &CancellationToken,
    ) -> Result<Computer, Error> {
        self.check(reference)?;
        self.computer(
            self.json(
                generated::WakeComputerParams {
                    id: reference.id.clone(),
                }
                .into_request()
                .map_err(|_| Error::InvalidInput)?,
                cancel,
            )
            .await?,
            Some(&reference.id),
        )
        .map_err(|cause| Error::Uncertain {
            operation: "wakeComputer",
            key: None,
            cause: Box::new(cause),
        })
    }
    /// Delete only a cloud computer. Never reinterpret its ID as an enrolled name.
    pub async fn delete(
        &self,
        reference: &ComputerRef,
        cancel: &CancellationToken,
    ) -> Result<(), Error> {
        self.check(reference)?;
        let result = self
            .request(
                generated::DeleteComputerParams {
                    id: reference.id.clone(),
                    kind: Some("cloud".into()),
                }
                .into_request()
                .map_err(|_| Error::InvalidInput)?,
                cancel,
            )
            .await?;
        if result.bytes.is_empty() {
            Ok(())
        } else {
            Err(Error::Uncertain {
                operation: "deleteComputer",
                key: None,
                cause: Box::new(Error::InvalidResponse),
            })
        }
    }

    /// Wait for an observed state; timeout and cancellation send no remote intent.
    pub async fn wait(
        &self,
        reference: &ComputerRef,
        target: WaitFor,
        timeout: Duration,
        cancel: &CancellationToken,
    ) -> Result<Computer, Error> {
        if timeout.is_zero() {
            return Err(Error::WaitTimedOut);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(Error::InvalidInput)?;
        let polling = async {
            loop {
                let computer = self.get(reference, cancel).await?;
                let ready = match target {
                    WaitFor::Awake => computer.awake(),
                    WaitFor::Stopped => computer.stopped(),
                    WaitFor::Deleted => {
                        computer.data.state == "deleted" && computer.data.desired == "deleted"
                    }
                };
                if ready {
                    return Ok(computer);
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        };
        tokio::select! {biased;_=cancel.cancelled()=>Err(Error::Cancelled),_=tokio::time::sleep_until(deadline)=>Err(Error::WaitTimedOut),value=polling=>value}
    }
}
