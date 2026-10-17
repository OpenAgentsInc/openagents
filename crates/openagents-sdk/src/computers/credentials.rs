//! Prepare credentials for a saved command without carrying their values.
use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct CredentialRequest {
    v: String,
    computer: ComputerRef,
    key: RequestKey,
    body: generated::AppPrepareComputerCredentialReference,
}
impl CredentialRequest {
    pub fn key(&self) -> &RequestKey {
        &self.key
    }
    pub fn computer(&self) -> &ComputerRef {
        &self.computer
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRef {
    computer: ComputerRef,
    id: String,
}
impl CredentialRef {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn computer(&self) -> &ComputerRef {
        &self.computer
    }
    /// The non-secret marker to give the model or Git client in the guest.
    pub fn placeholder(&self) -> String {
        format!("oa-ref:{}", self.id)
    }
}
#[derive(Clone)]
pub struct Credential {
    reference: CredentialRef,
    data: generated::AppComputerCredentialReference,
}
impl Credential {
    pub fn reference(&self) -> &CredentialRef {
        &self.reference
    }
    pub fn data(&self) -> &generated::AppComputerCredentialReference {
        &self.data
    }
}
opaque_debug!(CredentialRequest, CredentialRef, Credential);
fn credential_id(value: &str) -> Result<(), Error> {
    if value.len() != 36
        || !value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
impl Computers<'_> {
    /// Save this request, prepare every required source, then send the command.
    /// A wake or expired/revoked reference requires a new preparation request.
    pub fn credential_request(
        &self,
        command: &CommandRequest,
        source: &str,
        rule: &str,
        ttl_seconds: u64,
    ) -> Result<CredentialRequest, Error> {
        self.check(&command.computer)?;
        credential_id(source)?;
        if rule.is_empty()
            || rule.len() > 64
            || !rule
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            || !(30..=3600).contains(&ttl_seconds)
        {
            return Err(Error::InvalidInput);
        }
        let key = RequestKey::generate()?;
        Ok(CredentialRequest {
            v: "openagents.computers.credential-request.v1".into(),
            computer: command.computer.clone(),
            body: generated::AppPrepareComputerCredentialReference {
                source: source.to_ascii_lowercase(),
                rule: rule.into(),
                command_key: command.key.as_str().into(),
                idempotency_key: key.as_str().into(),
                ttl_seconds,
                extra: Default::default(),
            },
            key,
        })
    }
    pub fn credential_reference(
        &self,
        computer: &ComputerRef,
        id: &str,
    ) -> Result<CredentialRef, Error> {
        self.check(computer)?;
        credential_id(id)?;
        Ok(CredentialRef {
            computer: computer.clone(),
            id: id.to_ascii_lowercase(),
        })
    }
    fn credential(
        &self,
        computer: &ComputerRef,
        data: generated::AppComputerCredentialReference,
        expected: Option<&str>,
    ) -> Result<Credential, Error> {
        if data.computer != computer.id
            || expected.is_some_and(|id| id != data.id)
            || data.expires_unix <= data.created_unix
            || data.expires_unix - data.created_unix > 3600
        {
            return Err(Error::InvalidResponse);
        }
        let reference = self
            .credential_reference(computer, &data.id)
            .map_err(|_| Error::InvalidResponse)?;
        credential_id(&data.source).map_err(|_| Error::InvalidResponse)?;
        Ok(Credential { reference, data })
    }
    /// Add the non-secret marker before saving and dispatching the final command.
    /// A changed command body cannot reuse a key that was already submitted.
    pub fn command_with_credential(
        &self,
        command: &CommandRequest,
        credential: &Credential,
        env_name: &str,
    ) -> Result<CommandRequest, Error> {
        self.check(&command.computer)?;
        self.check(&credential.reference.computer)?;
        if command.computer != credential.reference.computer
            || credential.data.command_key != command.key.as_str()
            || !matches!(credential.data.state.as_str(), "prepared" | "active")
            || !env_name.starts_with("OA_PARAM_")
            || env_name.len() > 128
            || !env_name
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err(Error::InvalidInput);
        }
        let mut result = command.clone();
        let env = result.body.env.get_or_insert_default();
        env.insert(env_name.into(), credential.reference.placeholder());
        if env.len() > 16 {
            return Err(Error::InvalidInput);
        }
        Ok(result)
    }

    pub async fn prepare_credential(
        &self,
        request: &CredentialRequest,
        cancel: &CancellationToken,
    ) -> Result<Credential, Error> {
        self.check(&request.computer)?;
        if request.v != "openagents.computers.credential-request.v1"
            || request.body.idempotency_key != request.key.as_str()
            || !request.body.extra.is_empty()
        {
            return Err(Error::InvalidInput);
        }
        credential_id(&request.body.source)?;
        let plan = generated::PrepareComputerCredentialReferenceParams {
            id: request.computer.id.clone(),
            idempotency_key: Some(request.key.as_str().into()),
            body: request.body.clone(),
        }
        .into_request()
        .map_err(|_| Error::InvalidInput)?;
        let data: generated::AppComputerCredentialReference = self.json(plan, cancel).await?;
        let result = if data.source != request.body.source
            || data.rule != request.body.rule
            || data.command_key != request.body.command_key
        {
            Err(Error::InvalidResponse)
        } else {
            self.credential(&request.computer, data, None)
        };
        result.map_err(|cause| Error::Uncertain {
            operation: "prepareComputerCredentialReference",
            key: Some(request.key.as_str().into()),
            cause: Box::new(cause),
        })
    }
    pub async fn get_credential(
        &self,
        reference: &CredentialRef,
        cancel: &CancellationToken,
    ) -> Result<Credential, Error> {
        self.check(&reference.computer)?;
        credential_id(&reference.id)?;
        let plan = generated::GetComputerCredentialReferenceParams {
            id: reference.computer.id.clone(),
            reference: reference.id.clone(),
        }
        .into_request()
        .map_err(|_| Error::InvalidInput)?;
        let data = self.json(plan, cancel).await?;
        self.credential(&reference.computer, data, Some(&reference.id))
    }
    pub async fn revoke_credential(
        &self,
        reference: &CredentialRef,
        cancel: &CancellationToken,
    ) -> Result<(), Error> {
        self.check(&reference.computer)?;
        credential_id(&reference.id)?;
        let plan = generated::RevokeComputerCredentialReferenceParams {
            id: reference.computer.id.clone(),
            reference: reference.id.clone(),
        }
        .into_request()
        .map_err(|_| Error::InvalidInput)?;
        self.request(plan, cancel).await?;
        Ok(())
    }
}
