#[derive(Clone)]
struct Executor {
    runtime: Arc<tokio::runtime::Runtime>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Executor {
    fn new() -> Result<Self, ComputeError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| ComputeError::ProviderError(err.to_string()))?;
        Ok(Self {
            runtime: Arc::new(runtime),
        })
    }

    fn runtime(&self) -> Arc<tokio::runtime::Runtime> {
        self.runtime.clone()
    }

    fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        self.runtime.block_on(fut)
    }

    fn spawn<F>(&self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(fut);
    }
}

fn parse_prompt(input: &serde_json::Value) -> Option<String> {
    match input {
        serde_json::Value::String(prompt) => Some(prompt.clone()),
        serde_json::Value::Object(map) => map
            .get("prompt")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

fn parse_messages(input: &serde_json::Value) -> Option<String> {
    let messages = input.get("messages")?.as_array()?;
    let mut prompt = String::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("user");
        let content = message
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        prompt.push_str(role);
        prompt.push_str(": ");
        prompt.push_str(content);
        prompt.push('\n');
    }
    Some(prompt)
}
