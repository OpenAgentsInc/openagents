/// OpenAgents API auth response payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiAuthResponse {
    #[serde(flatten, default)]
    pub state: ApiAuthState,
    #[serde(default)]
    pub access_token: Option<String>,
}

/// OpenAgents API client interface for auth and container calls.
pub trait OpenAgentsApiClient: Send + Sync {
    fn authenticate_token(&self, token: &str) -> Result<ApiAuthResponse, ContainerError>;
    fn authenticate_nostr(
        &self,
        response: &NostrAuthResponse,
    ) -> Result<ApiAuthResponse, ContainerError>;
    fn provider_info(
        &self,
        provider_id: &str,
        token: Option<&str>,
    ) -> Result<ContainerProviderInfo, ContainerError>;
    fn submit_container(
        &self,
        provider_id: &str,
        request: &ContainerRequest,
        token: &str,
    ) -> Result<String, ContainerError>;
    fn session_state(&self, session_id: &str, token: &str) -> Result<SessionState, ContainerError>;
    fn submit_exec(
        &self,
        session_id: &str,
        command: &str,
        token: &str,
    ) -> Result<String, ContainerError>;
    fn exec_state(&self, exec_id: &str, token: &str) -> Result<ExecState, ContainerError>;
    fn poll_output(
        &self,
        session_id: &str,
        cursor: Option<&str>,
        token: &str,
    ) -> Result<(Option<OutputChunk>, Option<String>), ContainerError>;
    fn poll_exec_output(
        &self,
        exec_id: &str,
        cursor: Option<&str>,
        token: &str,
    ) -> Result<(Option<OutputChunk>, Option<String>), ContainerError>;
    fn read_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        len: u64,
        token: &str,
    ) -> Result<Vec<u8>, ContainerError>;
    fn write_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        data: &[u8],
        token: &str,
    ) -> Result<(), ContainerError>;
    fn stop(&self, session_id: &str, token: &str) -> Result<(), ContainerError>;
}

fn openagents_api_from_env() -> Option<Arc<dyn OpenAgentsApiClient>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        return HttpOpenAgentsApiClient::from_env();
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// OpenAgents API auth + credits manager.
#[derive(Clone)]
pub struct OpenAgentsAuth {
    agent_id: AgentId,
    storage: Arc<dyn AgentStorage>,
    signer: Arc<dyn SigningService>,
    api: Option<Arc<dyn OpenAgentsApiClient>>,
    api_base_url: Option<String>,
    state: Arc<RwLock<ApiAuthState>>,
    token_cache: Arc<Mutex<Option<String>>>,
    challenge: Arc<RwLock<Option<NostrAuthChallenge>>>,
}

impl OpenAgentsAuth {
    pub fn new(
        agent_id: AgentId,
        storage: Arc<dyn AgentStorage>,
        signer: Arc<dyn SigningService>,
        api: Option<Arc<dyn OpenAgentsApiClient>>,
    ) -> Self {
        let mut state = Self::load_state(&storage, &agent_id);
        let token = Self::load_token(&storage, &agent_id);
        state.token_set = token.is_some();
        if state.agent_pubkey.is_none() {
            if let Ok(npub) = Self::agent_npub_static(&signer, &agent_id) {
                state.agent_pubkey = Some(npub);
            }
        }
        let challenge = Self::load_challenge(&storage, &agent_id);
        Self {
            agent_id,
            storage,
            signer,
            api,
            api_base_url: None,
            state: Arc::new(RwLock::new(state)),
            token_cache: Arc::new(Mutex::new(token)),
            challenge: Arc::new(RwLock::new(challenge)),
        }
    }

    pub fn from_env(
        agent_id: AgentId,
        storage: Arc<dyn AgentStorage>,
        signer: Arc<dyn SigningService>,
    ) -> Self {
        let api = openagents_api_from_env();
        Self::new(agent_id, storage, signer, api)
    }

    /// Create auth state with an explicit OpenAgents API base URL (browser usage).
    pub fn with_base_url(
        agent_id: AgentId,
        storage: Arc<dyn AgentStorage>,
        signer: Arc<dyn SigningService>,
        base_url: impl Into<String>,
    ) -> Self {
        let mut auth = Self::new(agent_id, storage, signer, None);
        auth.api_base_url = Some(base_url.into());
        auth
    }

    pub fn status(&self) -> ApiAuthState {
        self.state.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn status_json(&self) -> FsResult<Vec<u8>> {
        serde_json::to_vec_pretty(&self.status()).map_err(|err| FsError::Other(err.to_string()))
    }

    pub fn credits_json(&self) -> FsResult<Vec<u8>> {
        let state = self.status();
        let json = serde_json::json!({ "credits_usd": state.credits_usd });
        serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))
    }

    pub fn challenge_json(&self) -> FsResult<Vec<u8>> {
        let challenge = self
            .issue_challenge()
            .map_err(|err| FsError::Other(err.to_string()))?;
        serde_json::to_vec_pretty(&challenge).map_err(|err| FsError::Other(err.to_string()))
    }

    pub fn token(&self) -> Option<String> {
        let mut cache = self.token_cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(token) = cache.as_ref() {
            return Some(token.clone());
        }
        let token = Self::load_token(&self.storage, &self.agent_id);
        *cache = token.clone();
        token
    }

    pub fn set_token(&self, token: &str) -> Result<(), ContainerError> {
        Self::store_token(&self.storage, &self.agent_id, token)?;
        {
            let mut cache = self.token_cache.lock().unwrap_or_else(|e| e.into_inner());
            *cache = Some(token.to_string());
        }
        let mut state = self.status();
        state.method = Some(AuthMethod::ApiKey);
        state.token_set = true;
        state.authenticated = false;
        state.expires_at = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(api) = &self.api {
                if let Ok(response) = api.authenticate_token(token) {
                    state = self.apply_auth_response(response, AuthMethod::ApiKey, None)?;
                }
            }
        }
        #[cfg(all(feature = "browser", target_arch = "wasm32"))]
        {
            if let Some(base_url) = self.api_base_url.clone() {
                let auth = self.clone();
                let token = token.trim().to_string();
                spawn_local(async move {
                    auth.validate_token_async(base_url, token).await;
                });
            }
        }
        self.save_state(&state)?;
        Ok(())
    }

    pub fn issue_challenge(&self) -> Result<NostrAuthChallenge, ContainerError> {
        let mut guard = self.challenge.write().unwrap_or_else(|e| e.into_inner());
        if let Some(challenge) = guard.as_ref() {
            if challenge.expires_at.as_millis() > Timestamp::now().as_millis() {
                return Ok(challenge.clone());
            }
        }
        let challenge = NostrAuthChallenge {
            challenge: uuid::Uuid::new_v4().to_string(),
            expires_at: Timestamp::from_millis(
                Timestamp::now()
                    .as_millis()
                    .saturating_add(AUTH_CHALLENGE_TTL.as_millis() as u64),
            ),
            pubkey: self.agent_npub()?,
        };
        Self::store_challenge(&self.storage, &self.agent_id, &challenge)?;
        *guard = Some(challenge.clone());
        Ok(challenge)
    }

    pub fn submit_challenge(&self, response: NostrAuthResponse) -> Result<(), ContainerError> {
        let challenge = self
            .load_current_challenge()
            .ok_or_else(|| ContainerError::InvalidRequest("no auth challenge".to_string()))?;
        if challenge.challenge != response.challenge {
            return Err(ContainerError::InvalidRequest(
                "challenge mismatch".to_string(),
            ));
        }
        if challenge.expires_at.as_millis() <= Timestamp::now().as_millis() {
            return Err(ContainerError::InvalidRequest(
                "challenge expired".to_string(),
            ));
        }
        let pubkey = Self::parse_pubkey(&response.pubkey)?;
        let signature_bytes = hex::decode(&response.signature)
            .map_err(|_| ContainerError::InvalidRequest("invalid signature".to_string()))?;
        let signature = Signature::new(signature_bytes);
        if !self
            .signer
            .verify(&pubkey, response.challenge.as_bytes(), &signature)
        {
            return Err(ContainerError::InvalidRequest(
                "signature verification failed".to_string(),
            ));
        }
        let expected_pubkey = self
            .signer
            .pubkey(&self.agent_id)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        if expected_pubkey.as_bytes() != pubkey.as_bytes() {
            return Err(ContainerError::InvalidRequest(
                "pubkey mismatch".to_string(),
            ));
        }

        let mut state = self.status();
        state.method = Some(AuthMethod::Nostr);
        state.agent_pubkey = Some(challenge.pubkey.clone());
        state.authenticated = true;
        state.expires_at = None;
        state.rate_limit = RateLimitStatus::default();
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(api) = &self.api {
                if let Ok(response) = api.authenticate_nostr(&response) {
                    state = self.apply_auth_response(
                        response,
                        AuthMethod::Nostr,
                        Some(challenge.pubkey.clone()),
                    )?;
                }
            }
        }
        #[cfg(all(feature = "browser", target_arch = "wasm32"))]
        {
            if let Some(base_url) = self.api_base_url.clone() {
                let auth = self.clone();
                spawn_local(async move {
                    auth.validate_nostr_async(base_url, response).await;
                });
            }
        }
        self.save_state(&state)?;
        Self::clear_challenge(&self.storage, &self.agent_id)?;
        let mut guard = self.challenge.write().unwrap_or_else(|e| e.into_inner());
        *guard = None;
        Ok(())
    }

    fn apply_auth_response(
        &self,
        response: ApiAuthResponse,
        default_method: AuthMethod,
        default_pubkey: Option<String>,
    ) -> Result<ApiAuthState, ContainerError> {
        let mut state = response.state;
        if state.method.is_none() {
            state.method = Some(default_method);
        }
        if state.agent_pubkey.is_none() {
            state.agent_pubkey = default_pubkey;
        }
        if let Some(access_token) = response.access_token {
            Self::store_token(&self.storage, &self.agent_id, &access_token)?;
            let mut cache = self.token_cache.lock().unwrap_or_else(|e| e.into_inner());
            *cache = Some(access_token);
        }
        state.token_set = self.token().is_some();
        Ok(state)
    }

    #[cfg(all(feature = "browser", target_arch = "wasm32"))]
    async fn validate_token_async(&self, base_url: String, token: String) {
        let url = format!("{}/containers/auth/token", base_url.trim_end_matches('/'));
        let body = match serde_json::to_string(&serde_json::json!({ "token": token })) {
            Ok(body) => body,
            Err(_) => return,
        };
        let response = wasm_http::request_bytes("POST", &url, None, Some(body)).await;
        let Ok((status, bytes)) = response else {
            return;
        };
        if !(200..300).contains(&status) {
            return;
        }
        let Ok(payload) = serde_json::from_slice::<ApiAuthResponse>(&bytes) else {
            return;
        };
        let state = match self.apply_auth_response(payload, AuthMethod::ApiKey, None) {
            Ok(state) => state,
            Err(_) => return,
        };
        let _ = self.save_state(&state);
    }

    #[cfg(all(feature = "browser", target_arch = "wasm32"))]
    async fn validate_nostr_async(&self, base_url: String, response: NostrAuthResponse) {
        let url = format!("{}/containers/auth/nostr", base_url.trim_end_matches('/'));
        let body = match serde_json::to_string(&response) {
            Ok(body) => body,
            Err(_) => return,
        };
        let resp = wasm_http::request_bytes("POST", &url, None, Some(body)).await;
        let Ok((status, bytes)) = resp else {
            return;
        };
        if !(200..300).contains(&status) {
            return;
        }
        let Ok(payload) = serde_json::from_slice::<ApiAuthResponse>(&bytes) else {
            return;
        };
        let state = match self
            .apply_auth_response(payload, AuthMethod::Nostr, Some(response.pubkey.clone()))
        {
            Ok(state) => state,
            Err(_) => return,
        };
        let _ = self.save_state(&state);
    }

    pub fn check_auth(
        &self,
        provider_id: &str,
        policy: &ContainerPolicy,
        requires_auth: bool,
    ) -> Result<(), ContainerError> {
        let state = self.status();
        if provider_id == "local" {
            return Ok(());
        }
        if !policy.require_api_auth && !requires_auth {
            return Ok(());
        }
        if !state.authenticated {
            return Err(ContainerError::AuthRequired {
                provider: provider_id.to_string(),
                message: "OpenAgents API authentication required".to_string(),
            });
        }
        if provider_id != "local" && !state.token_set {
            return Err(ContainerError::AuthRequired {
                provider: provider_id.to_string(),
                message: "OpenAgents API token required".to_string(),
            });
        }
        if state.rate_limit.is_limited() && state.rate_limit.remaining == 0 {
            return Err(ContainerError::RateLimited {
                resets_at: state.rate_limit.resets_at,
            });
        }
        Ok(())
    }

    pub fn check_credits(&self, estimated_cost_usd: u64) -> Result<(), ContainerError> {
        let state = self.status();
        if state.credits_usd < estimated_cost_usd {
            return Err(ContainerError::InsufficientCredits {
                required_usd: estimated_cost_usd,
                available_usd: state.credits_usd,
            });
        }
        Ok(())
    }

    pub fn reserve_credits(&self, amount: u64) -> Result<u64, ContainerError> {
        if amount == 0 {
            return Ok(0);
        }
        let mut state = self.status();
        if state.credits_usd < amount {
            return Err(ContainerError::InsufficientCredits {
                required_usd: amount,
                available_usd: state.credits_usd,
            });
        }
        state.credits_usd = state.credits_usd.saturating_sub(amount);
        self.save_state(&state)?;
        Ok(amount)
    }

    pub fn release_credits(&self, amount: u64) -> Result<(), ContainerError> {
        if amount == 0 {
            return Ok(());
        }
        let mut state = self.status();
        state.credits_usd = state.credits_usd.saturating_add(amount);
        self.save_state(&state)?;
        Ok(())
    }

    pub fn reconcile_credits(&self, reserved: u64, actual: u64) -> Result<(), ContainerError> {
        if reserved == 0 {
            return Ok(());
        }
        let mut state = self.status();
        if actual <= reserved {
            state.credits_usd = state.credits_usd.saturating_add(reserved - actual);
        } else {
            let extra = actual - reserved;
            state.credits_usd = state.credits_usd.saturating_sub(extra);
        }
        self.save_state(&state)?;
        Ok(())
    }

    fn agent_npub(&self) -> Result<String, ContainerError> {
        Self::agent_npub_static(&self.signer, &self.agent_id)
    }

    fn agent_npub_static(
        signer: &Arc<dyn SigningService>,
        agent_id: &AgentId,
    ) -> Result<String, ContainerError> {
        let pubkey = signer
            .pubkey(agent_id)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let bytes = pubkey.as_bytes();
        if bytes.len() != 32 {
            return Err(ContainerError::InvalidRequest(
                "nostr pubkey must be 32 bytes".to_string(),
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        public_key_to_npub(&arr)
    }

    fn parse_pubkey(value: &str) -> Result<PublicKey, ContainerError> {
        if value.starts_with("npub1") {
            let bytes = npub_to_public_key(value)?;
            return Ok(PublicKey::new(bytes.to_vec()));
        }
        let bytes = hex::decode(value)
            .map_err(|_| ContainerError::InvalidRequest("invalid pubkey".to_string()))?;
        if bytes.len() != 32 {
            return Err(ContainerError::InvalidRequest(
                "nostr pubkey must be 32 bytes".to_string(),
            ));
        }
        Ok(PublicKey::new(bytes))
    }

    fn load_state(storage: &Arc<dyn AgentStorage>, agent_id: &AgentId) -> ApiAuthState {
        let data = futures::executor::block_on(storage.get(agent_id, AUTH_STATE_KEY))
            .ok()
            .flatten();
        data.and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save_state(&self, state: &ApiAuthState) -> Result<(), ContainerError> {
        let data = serde_json::to_vec_pretty(state)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        futures::executor::block_on(self.storage.set(&self.agent_id, AUTH_STATE_KEY, &data))
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let mut guard = self.state.write().unwrap_or_else(|e| e.into_inner());
        *guard = state.clone();
        Ok(())
    }

    fn load_token(storage: &Arc<dyn AgentStorage>, agent_id: &AgentId) -> Option<String> {
        let data = futures::executor::block_on(storage.get(agent_id, AUTH_TOKEN_KEY))
            .ok()
            .flatten()?;
        String::from_utf8(data).ok()
    }

    fn store_token(
        storage: &Arc<dyn AgentStorage>,
        agent_id: &AgentId,
        token: &str,
    ) -> Result<(), ContainerError> {
        futures::executor::block_on(storage.set(agent_id, AUTH_TOKEN_KEY, token.as_bytes()))
            .map_err(|err| ContainerError::ProviderError(err.to_string()))
    }

    fn load_challenge(
        storage: &Arc<dyn AgentStorage>,
        agent_id: &AgentId,
    ) -> Option<NostrAuthChallenge> {
        let data = futures::executor::block_on(storage.get(agent_id, AUTH_CHALLENGE_KEY))
            .ok()
            .flatten()?;
        serde_json::from_slice(&data).ok()
    }

    fn store_challenge(
        storage: &Arc<dyn AgentStorage>,
        agent_id: &AgentId,
        challenge: &NostrAuthChallenge,
    ) -> Result<(), ContainerError> {
        let data = serde_json::to_vec(challenge)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        futures::executor::block_on(storage.set(agent_id, AUTH_CHALLENGE_KEY, &data))
            .map_err(|err| ContainerError::ProviderError(err.to_string()))
    }

    fn clear_challenge(
        storage: &Arc<dyn AgentStorage>,
        agent_id: &AgentId,
    ) -> Result<(), ContainerError> {
        futures::executor::block_on(storage.delete(agent_id, AUTH_CHALLENGE_KEY))
            .map_err(|err| ContainerError::ProviderError(err.to_string()))
    }

    fn load_current_challenge(&self) -> Option<NostrAuthChallenge> {
        let guard = self.challenge.read().unwrap_or_else(|e| e.into_inner());
        if guard.as_ref().is_some() {
            return guard.clone();
        }
        drop(guard);
        let challenge = Self::load_challenge(&self.storage, &self.agent_id);
        let mut guard = self.challenge.write().unwrap_or_else(|e| e.into_inner());
        *guard = challenge.clone();
        challenge
    }
}

fn npub_to_public_key(value: &str) -> Result<[u8; 32], ContainerError> {
    let (hrp, data) = bech32::decode(value)
        .map_err(|_| ContainerError::InvalidRequest("invalid npub".to_string()))?;
    if hrp.as_str() != "npub" {
        return Err(ContainerError::InvalidRequest(
            "invalid npub prefix".to_string(),
        ));
    }
    if data.len() != 32 {
        return Err(ContainerError::InvalidRequest(
            "nostr pubkey must be 32 bytes".to_string(),
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&data);
    Ok(out)
}

fn public_key_to_npub(public_key: &[u8; 32]) -> Result<String, ContainerError> {
    let hrp = Hrp::parse("npub").map_err(|err| ContainerError::ProviderError(err.to_string()))?;
    bech32::encode::<Bech32>(hrp, public_key)
        .map_err(|err| ContainerError::ProviderError(err.to_string()))
}

impl ApiTokenProvider for OpenAgentsAuth {
    fn api_token(&self) -> Option<String> {
        self.token()
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct HttpOpenAgentsApiClient {
    base_url: String,
    client: reqwest::Client,
}

#[cfg(not(target_arch = "wasm32"))]
impl HttpOpenAgentsApiClient {
    fn from_env() -> Option<Arc<dyn OpenAgentsApiClient>> {
        let base_url = std::env::var(OPENAGENTS_API_URL_ENV).ok()?;
        if base_url.trim().is_empty() {
            return None;
        }
        Self::new(base_url)
            .ok()
            .map(|client| Arc::new(client) as Arc<dyn OpenAgentsApiClient>)
    }

    fn new(base_url: impl Into<String>) -> Result<Self, ContainerError> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        Ok(Self {
            base_url: base_url.into(),
            client,
        })
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn build_request(
        &self,
        method: reqwest::Method,
        path: &str,
        token: Option<&str>,
    ) -> reqwest::RequestBuilder {
        let mut builder = self.client.request(method, self.url(path));
        if let Some(token) = token {
            builder = builder.bearer_auth(token);
        }
        builder
    }

    fn execute<F, T>(&self, fut: F) -> Result<T, ContainerError>
    where
        F: std::future::Future<Output = Result<T, ContainerError>>,
    {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            return tokio::task::block_in_place(|| handle.block_on(fut));
        }
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        runtime.block_on(fut)
    }

    fn send_request(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> Result<(reqwest::StatusCode, Vec<u8>), ContainerError> {
        self.execute(async {
            let response = builder
                .send()
                .await
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
            let status = response.status();
            let bytes = response
                .bytes()
                .await
                .map_err(|err| ContainerError::ProviderError(err.to_string()))?
                .to_vec();
            Ok((status, bytes))
        })
    }

    fn request_json<R: DeserializeOwned>(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> Result<R, ContainerError> {
        let (status, bytes) = self.send_request(builder)?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        serde_json::from_slice(&bytes).map_err(|err| ContainerError::ProviderError(err.to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl OpenAgentsApiClient for HttpOpenAgentsApiClient {
    fn authenticate_token(&self, token: &str) -> Result<ApiAuthResponse, ContainerError> {
        let body = serde_json::json!({ "token": token });
        let builder = self
            .build_request(reqwest::Method::POST, "containers/auth/token", None)
            .json(&body);
        self.request_json(builder)
    }

    fn authenticate_nostr(
        &self,
        response: &NostrAuthResponse,
    ) -> Result<ApiAuthResponse, ContainerError> {
        let builder = self
            .build_request(reqwest::Method::POST, "containers/auth/nostr", None)
            .json(response);
        self.request_json(builder)
    }

    fn provider_info(
        &self,
        provider_id: &str,
        token: Option<&str>,
    ) -> Result<ContainerProviderInfo, ContainerError> {
        let path = format!("containers/providers/{}/info", provider_id);
        let builder = self.build_request(reqwest::Method::GET, &path, token);
        self.request_json(builder)
    }

    fn submit_container(
        &self,
        provider_id: &str,
        request: &ContainerRequest,
        token: &str,
    ) -> Result<String, ContainerError> {
        #[derive(Deserialize)]
        struct SessionResponse {
            session_id: String,
        }
        let path = format!("containers/providers/{}/sessions", provider_id);
        let body = serde_json::to_value(request)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        let builder = self
            .build_request(reqwest::Method::POST, &path, Some(token))
            .json(&body);
        let response: SessionResponse = self.request_json(builder)?;
        Ok(response.session_id)
    }

    fn session_state(&self, session_id: &str, token: &str) -> Result<SessionState, ContainerError> {
        let path = format!("containers/sessions/{}", session_id);
        let builder = self.build_request(reqwest::Method::GET, &path, Some(token));
        let (status, bytes) = self.send_request(builder)?;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(ContainerError::SessionNotFound);
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        serde_json::from_slice(&bytes).map_err(|err| ContainerError::ProviderError(err.to_string()))
    }

    fn submit_exec(
        &self,
        session_id: &str,
        command: &str,
        token: &str,
    ) -> Result<String, ContainerError> {
        #[derive(Deserialize)]
        struct ExecResponse {
            exec_id: String,
        }
        let path = format!("containers/sessions/{}/exec", session_id);
        let body = serde_json::json!({ "command": command });
        let builder = self
            .build_request(reqwest::Method::POST, &path, Some(token))
            .json(&body);
        let response: ExecResponse = self.request_json(builder)?;
        Ok(response.exec_id)
    }

    fn exec_state(&self, exec_id: &str, token: &str) -> Result<ExecState, ContainerError> {
        let path = format!("containers/exec/{}", exec_id);
        let builder = self.build_request(reqwest::Method::GET, &path, Some(token));
        let (status, bytes) = self.send_request(builder)?;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(ContainerError::ExecNotFound);
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        serde_json::from_slice(&bytes).map_err(|err| ContainerError::ProviderError(err.to_string()))
    }

    fn poll_output(
        &self,
        session_id: &str,
        cursor: Option<&str>,
        token: &str,
    ) -> Result<(Option<OutputChunk>, Option<String>), ContainerError> {
        #[derive(Deserialize)]
        struct OutputResponse {
            chunk: Option<OutputChunk>,
            cursor: Option<String>,
        }
        let mut path = format!("containers/sessions/{}/output", session_id);
        if let Some(cursor) = cursor {
            path = format!("{}?cursor={}", path, cursor);
        }
        let builder = self.build_request(reqwest::Method::GET, &path, Some(token));
        let (status, bytes) = self.send_request(builder)?;
        if status == reqwest::StatusCode::NO_CONTENT || bytes.is_empty() {
            return Ok((None, cursor.map(|c| c.to_string())));
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        if let Ok(payload) = serde_json::from_slice::<OutputResponse>(&bytes) {
            return Ok((payload.chunk, payload.cursor));
        }
        let chunk: OutputChunk = serde_json::from_slice(&bytes)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        Ok((Some(chunk), None))
    }

    fn poll_exec_output(
        &self,
        exec_id: &str,
        cursor: Option<&str>,
        token: &str,
    ) -> Result<(Option<OutputChunk>, Option<String>), ContainerError> {
        #[derive(Deserialize)]
        struct OutputResponse {
            chunk: Option<OutputChunk>,
            cursor: Option<String>,
        }
        let mut path = format!("containers/exec/{}/output", exec_id);
        if let Some(cursor) = cursor {
            path = format!("{}?cursor={}", path, cursor);
        }
        let builder = self.build_request(reqwest::Method::GET, &path, Some(token));
        let (status, bytes) = self.send_request(builder)?;
        if status == reqwest::StatusCode::NO_CONTENT || bytes.is_empty() {
            return Ok((None, cursor.map(|c| c.to_string())));
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        if let Ok(payload) = serde_json::from_slice::<OutputResponse>(&bytes) {
            return Ok((payload.chunk, payload.cursor));
        }
        let chunk: OutputChunk = serde_json::from_slice(&bytes)
            .map_err(|err| ContainerError::ProviderError(err.to_string()))?;
        Ok((Some(chunk), None))
    }

    fn read_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        len: u64,
        token: &str,
    ) -> Result<Vec<u8>, ContainerError> {
        let encoded = encode(path);
        let path = format!(
            "containers/sessions/{}/files/{}?offset={}&len={}",
            session_id, encoded, offset, len
        );
        let builder = self.build_request(reqwest::Method::GET, &path, Some(token));
        let (status, bytes) = self.send_request(builder)?;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(ContainerError::SessionNotFound);
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        Ok(bytes)
    }

    fn write_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        data: &[u8],
        token: &str,
    ) -> Result<(), ContainerError> {
        let encoded = encode(path);
        let path = format!(
            "containers/sessions/{}/files/{}?offset={}",
            session_id, encoded, offset
        );
        let builder = self
            .build_request(reqwest::Method::PUT, &path, Some(token))
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(data.to_vec());
        let (status, bytes) = self.send_request(builder)?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        Ok(())
    }

    fn stop(&self, session_id: &str, token: &str) -> Result<(), ContainerError> {
        let path = format!("containers/sessions/{}/stop", session_id);
        let builder = self.build_request(reqwest::Method::POST, &path, Some(token));
        let (status, bytes) = self.send_request(builder)?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(ContainerError::ProviderError(format!(
                "openagents api {}: {}",
                status, body
            )));
        }
        Ok(())
    }
}

#[derive(Clone)]
struct SessionRecord {
    provider_id: String,
    reservation: BudgetReservation,
    reconciled: bool,
    credits_reserved: u64,
}

#[derive(Clone)]
struct ExecRecord {
    provider_id: String,
    session_id: String,
}

