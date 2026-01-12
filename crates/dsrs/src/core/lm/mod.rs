pub mod chat;
pub mod client_registry;
pub mod lm_router;
pub mod pylon;
pub mod usage;

pub use chat::*;
pub use client_registry::*;
pub use lm_router::*;
pub use pylon::*;
pub use usage::*;

use anyhow::Result;
use rig::{completion::AssistantContent, message::ToolCall, message::ToolChoice, tool::ToolDyn};

use bon::Builder;
use std::future::Future;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

use crate::{Cache, CallResult, Example, MetaSignature, Prediction, ResponseCache};
use crate::callbacks::DspyCallback;

#[derive(Clone, Debug)]
pub struct LMResponse {
    /// Assistant message chosen by the provider.
    pub output: Message,
    /// Token usage reported by the provider for this call.
    pub usage: LmUsage,
    /// Chat history including the freshly appended assistant response.
    pub chat: Chat,
    /// Tool calls made by the provider.
    pub tool_calls: Vec<ToolCall>,
    /// Tool executions made by the provider.
    pub tool_executions: Vec<String>,
}

#[derive(Builder)]
#[builder(finish_fn(vis = "", name = __internal_build))]
pub struct LM {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    #[builder(default = "openai:gpt-4o-mini".to_string())]
    pub model: String,
    #[builder(default = 0.7)]
    pub temperature: f32,
    #[builder(default = 512)]
    pub max_tokens: u32,
    #[builder(default = 10)]
    pub max_tool_iterations: u32,
    #[builder(default = false)]
    pub cache: bool,
    pub cache_handler: Option<Arc<Mutex<ResponseCache>>>,
    #[builder(skip)]
    client: Option<Arc<LMClient>>,
}

impl Default for LM {
    fn default() -> Self {
        block_on_detached(async { Self::builder().build().await.unwrap() })
    }
}

impl Clone for LM {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            max_tool_iterations: self.max_tool_iterations,
            cache: self.cache,
            cache_handler: self.cache_handler.clone(),
            client: self.client.clone(),
        }
    }
}

fn block_on_detached<F, T>(future: F) -> T
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new()
                .expect("failed to create runtime for detached blocking call");
            runtime.block_on(future)
        })
        .join()
        .expect("detached runtime thread panicked")
    } else {
        let runtime = tokio::runtime::Runtime::new()
            .expect("failed to create runtime for detached blocking call");
        runtime.block_on(future)
    }
}

impl LM {
    /// Finalizes construction of an [`LM`], initializing the HTTP client and
    /// optional response cache based on provided parameters.
    ///
    /// Supports 3 build cases:
    /// 1. OpenAI-compatible with auth: `base_url` + `api_key` provided
    ///    → Uses OpenAI client with custom base URL
    /// 2. Local OpenAI-compatible: `base_url` only (no `api_key`)
    ///    → Uses OpenAI client for vLLM/local servers (dummy key)
    /// 3. Provider via model string: no `base_url`, model in "provider:model" format
    ///    → Uses provider-specific client (openai, openai, gemini, etc.)
    async fn initialize_client(mut self) -> Result<Self> {
        // Determine which build case based on what's provided
        let client = match (&self.base_url, &self.api_key, &self.model) {
            // Case 1: OpenAI-compatible with authentication (base_url + api_key)
            // For custom OpenAI-compatible APIs that require API keys
            (Some(base_url), Some(api_key), _) => Arc::new(LMClient::from_openai_compatible(
                base_url,
                api_key,
                &self.model,
            )?),
            // Case 2: Local OpenAI-compatible server (base_url only, no api_key)
            // For vLLM, text-generation-inference, and other local OpenAI-compatible servers
            (Some(base_url), None, _) => Arc::new(LMClient::from_local(base_url, &self.model)?),
            // Case 3: Provider via model string (no base_url, model in "provider:model" format)
            // Uses provider-specific clients
            (None, api_key, model) if model.contains(':') => {
                Arc::new(LMClient::from_model_string(model, api_key.as_deref())?)
            }
            // Default case: assume OpenAI provider if no colon in model name
            (None, api_key, model) => {
                let model_str = if model.contains(':') {
                    model.to_string()
                } else {
                    format!("openai:{}", model)
                };
                Arc::new(LMClient::from_model_string(&model_str, api_key.as_deref())?)
            }
        };

        self.client = Some(client);

        // Initialize cache if enabled
        if self.cache && self.cache_handler.is_none() {
            self.cache_handler = Some(Arc::new(Mutex::new(ResponseCache::new().await)));
        }

        Ok(self)
    }

    pub async fn with_client(self, client: LMClient) -> Result<Self> {
        Ok(LM {
            client: Some(Arc::new(client)),
            ..self
        })
    }
}

// Implement build() for all builder states since optional fields don't require setting
impl<S: l_m_builder::State> LMBuilder<S> {
    /// Builds the LM instance with proper client initialization
    ///
    /// Supports 3 build cases:
    /// 1. OpenAI-compatible with auth: `base_url` + `api_key` provided
    /// 2. Local OpenAI-compatible: `base_url` only (for vLLM, etc.)
    /// 3. Provider via model string: model in "provider:model" format
    pub async fn build(self) -> Result<LM> {
        let lm = self.__internal_build();
        lm.initialize_client().await
    }
}

struct ToolLoopResult {
    message: Message,
    #[allow(unused)]
    chat_history: Vec<rig::message::Message>,
    tool_calls: Vec<ToolCall>,
    tool_executions: Vec<String>,
}

impl LM {
    async fn execute_tool_loop(
        &self,
        initial_tool_call: &rig::message::ToolCall,
        mut tools: Vec<Arc<dyn ToolDyn>>,
        tool_definitions: Vec<rig::completion::ToolDefinition>,
        mut chat_history: Vec<rig::message::Message>,
        system_prompt: String,
        accumulated_usage: &mut LmUsage,
    ) -> Result<ToolLoopResult> {
        use rig::OneOrMany;
        use rig::completion::CompletionRequest;
        use rig::message::UserContent;

        let max_iterations = self.max_tool_iterations as usize;

        let mut tool_calls = Vec::new();
        let mut tool_executions = Vec::new();

        // Execute the first tool call
        let tool_name = &initial_tool_call.function.name;
        let args_str = initial_tool_call.function.arguments.to_string();

        let mut tool_result = format!("Tool '{}' not found", tool_name);
        for tool in &mut tools {
            let def = tool.definition("".to_string()).await;
            if def.name == *tool_name {
                // Actually execute the tool
                tool_result = tool.call(args_str.clone()).await.unwrap();
                tool_calls.push(initial_tool_call.clone());
                tool_executions.push(tool_result.clone());
                break;
            }
        }

        // Add initial tool call and result to history
        chat_history.push(rig::message::Message::Assistant {
            id: None,
            content: OneOrMany::one(rig::message::AssistantContent::ToolCall(
                initial_tool_call.clone(),
            )),
        });

        let tool_result_content = if let Some(call_id) = &initial_tool_call.call_id {
            UserContent::tool_result_with_call_id(
                initial_tool_call.id.clone(),
                call_id.clone(),
                OneOrMany::one(tool_result.into()),
            )
        } else {
            UserContent::tool_result(
                initial_tool_call.id.clone(),
                OneOrMany::one(tool_result.into()),
            )
        };

        chat_history.push(rig::message::Message::User {
            content: OneOrMany::one(tool_result_content),
        });

        // Now loop until we get a text response
        for _iteration in 1..max_iterations {
            let request = CompletionRequest {
                preamble: Some(system_prompt.clone()),
                chat_history: if chat_history.len() == 1 {
                    OneOrMany::one(chat_history.clone().into_iter().next().unwrap())
                } else {
                    OneOrMany::many(chat_history.clone()).expect("chat_history should not be empty")
                },
                documents: Vec::new(),
                tools: tool_definitions.clone(),
                temperature: Some(self.temperature as f64),
                max_tokens: Some(self.max_tokens as u64),
                tool_choice: Some(ToolChoice::Auto),
                additional_params: None,
            };

            let response = self
                .client
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("LM client not initialized"))?
                .completion(request)
                .await?;

            accumulated_usage.prompt_tokens += response.usage.input_tokens;
            accumulated_usage.completion_tokens += response.usage.output_tokens;
            accumulated_usage.total_tokens += response.usage.total_tokens;

            match response.choice.first() {
                AssistantContent::Text(text) => {
                    return Ok(ToolLoopResult {
                        message: Message::assistant(&text.text),
                        chat_history,
                        tool_calls,
                        tool_executions,
                    });
                }
                AssistantContent::Reasoning(reasoning) => {
                    return Ok(ToolLoopResult {
                        message: Message::assistant(reasoning.reasoning.join("\n")),
                        chat_history,
                        tool_calls,
                        tool_executions,
                    });
                }
                AssistantContent::ToolCall(tool_call) => {
                    // Execute tool and continue
                    let tool_name = &tool_call.function.name;
                    let args_str = tool_call.function.arguments.to_string();

                    let mut tool_result = format!("Tool '{}' not found", tool_name);
                    for tool in &mut tools {
                        let def = tool.definition("".to_string()).await;
                        if def.name == *tool_name {
                            // Actually execute the tool
                            tool_result = tool.call(args_str.clone()).await.unwrap();
                            tool_calls.push(tool_call.clone());
                            tool_executions.push(tool_result.clone());
                            break;
                        }
                    }

                    chat_history.push(rig::message::Message::Assistant {
                        id: None,
                        content: OneOrMany::one(rig::message::AssistantContent::ToolCall(
                            tool_call.clone(),
                        )),
                    });

                    let tool_result_content = if let Some(call_id) = &tool_call.call_id {
                        UserContent::tool_result_with_call_id(
                            tool_call.id.clone(),
                            call_id.clone(),
                            OneOrMany::one(tool_result.into()),
                        )
                    } else {
                        UserContent::tool_result(
                            tool_call.id.clone(),
                            OneOrMany::one(tool_result.into()),
                        )
                    };

                    chat_history.push(rig::message::Message::User {
                        content: OneOrMany::one(tool_result_content),
                    });
                }
                AssistantContent::Image(_image) => todo!(),
            }
        }

        Err(anyhow::anyhow!("Max tool iterations reached"))
    }

    pub async fn call(&self, messages: Chat, tools: Vec<Arc<dyn ToolDyn>>) -> Result<LMResponse> {
        self.call_with_signature(None, messages, tools).await
    }

    pub async fn call_with_signature(
        &self,
        signature: Option<&dyn MetaSignature>,
        messages: Chat,
        tools: Vec<Arc<dyn ToolDyn>>,
    ) -> Result<LMResponse> {
        self.call_with_signature_streaming(signature, messages, tools, None).await
    }

    pub async fn call_with_signature_streaming(
        &self,
        signature: Option<&dyn MetaSignature>,
        messages: Chat,
        tools: Vec<Arc<dyn ToolDyn>>,
        callback: Option<&dyn DspyCallback>,
    ) -> Result<LMResponse> {
        use rig::OneOrMany;
        use rig::completion::CompletionRequest;

        let request_messages = messages.get_rig_messages();

        let mut tool_definitions = Vec::new();
        for tool in &tools {
            tool_definitions.push(tool.definition("".to_string()).await);
        }

        // Build the completion request manually
        let mut chat_history = request_messages.conversation;
        chat_history.push(request_messages.prompt);

        let request = CompletionRequest {
            preamble: Some(request_messages.system.clone()),
            chat_history: if chat_history.len() == 1 {
                OneOrMany::one(chat_history.clone().into_iter().next().unwrap())
            } else {
                OneOrMany::many(chat_history.clone()).expect("chat_history should not be empty")
            },
            documents: Vec::new(),
            tools: tool_definitions.clone(),
            temperature: Some(self.temperature as f64),
            max_tokens: Some(self.max_tokens as u64),
            tool_choice: if !tool_definitions.is_empty() {
                Some(ToolChoice::Auto)
            } else {
                None
            },
            additional_params: None,
        };

        let client = self.client.as_ref().ok_or_else(|| {
            anyhow::anyhow!("LM client not initialized. Call build() on LMBuilder.")
        })?;

        // Generate call ID for streaming callbacks
        let call_id = uuid::Uuid::new_v4();

        // Signal stream start
        if let Some(cb) = callback {
            cb.on_lm_stream_start(call_id, &self.model);
        }

        let (response, usage_override) = match client.as_ref() {
            LMClient::LmRouter(router) => {
                let completion = router
                    .completion_with_signature(signature, request)
                    .await?;
                (completion.response, Some(completion.usage))
            }
            _ => (client.completion(request).await?, None),
        };

        let mut accumulated_usage = usage_override.unwrap_or_else(|| LmUsage::from(response.usage));

        // Handle the response
        let mut tool_loop_result = None;
        let first_choice = match response.choice.first() {
            AssistantContent::Text(text) => {
                // Emit the text as a token for streaming
                if let Some(cb) = callback {
                    cb.on_lm_token(call_id, &text.text);
                }
                Message::assistant(&text.text)
            }
            AssistantContent::Reasoning(reasoning) => {
                let text = reasoning.reasoning.join("\n");
                if let Some(cb) = callback {
                    cb.on_lm_token(call_id, &text);
                }
                Message::assistant(text)
            }
            AssistantContent::ToolCall(tool_call) if !tools.is_empty() => {
                // Only execute tool loop if we have tools available
                let result = self
                    .execute_tool_loop(
                        &tool_call,
                        tools,
                        tool_definitions,
                        chat_history,
                        request_messages.system,
                        &mut accumulated_usage,
                    )
                    .await
                    .unwrap();
                let message = result.message.clone();
                tool_loop_result = Some(result);
                message
            }
            AssistantContent::ToolCall(tool_call) => {
                // No tools available, just return a message indicating this
                let msg = format!(
                    "Tool call requested: {} with args: {}, but no tools available",
                    tool_call.function.name, tool_call.function.arguments
                );
                Message::assistant(&msg)
            }
            AssistantContent::Image(_image) => todo!(),
        };

        let mut full_chat = messages.clone();
        full_chat.push_message(first_choice.clone());

        // Signal stream end
        if let Some(cb) = callback {
            cb.on_lm_stream_end(call_id);
        }

        Ok(LMResponse {
            output: first_choice,
            usage: accumulated_usage,
            chat: full_chat,
            tool_calls: tool_loop_result
                .as_ref()
                .map(|result| result.tool_calls.clone())
                .unwrap_or_default(),
            tool_executions: tool_loop_result
                .map(|result| result.tool_executions)
                .unwrap_or_default(),
        })
    }

    /// Returns the `n` most recent cached calls.
    ///
    /// Panics if caching is disabled for this `LM`.
    pub async fn inspect_history(&self, n: usize) -> Vec<CallResult> {
        self.cache_handler
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_history(n)
            .await
            .unwrap()
    }
}

/// In-memory LM used for deterministic tests and examples.
#[derive(Clone, Builder, Default)]
pub struct DummyLM {
    pub api_key: String,
    #[builder(default = "https://api.openai.com/v1".to_string())]
    pub base_url: String,
    #[builder(default = 0.7)]
    pub temperature: f32,
    #[builder(default = 512)]
    pub max_tokens: u32,
    #[builder(default = true)]
    pub cache: bool,
    /// Cache backing storage shared with the real implementation.
    pub cache_handler: Option<Arc<Mutex<ResponseCache>>>,
}

impl DummyLM {
    /// Creates a new [`DummyLM`] with an enabled in-memory cache.
    pub async fn new() -> Self {
        let cache_handler = Arc::new(Mutex::new(ResponseCache::new().await));
        Self {
            api_key: "".into(),
            base_url: "https://api.openai.com/v1".to_string(),
            temperature: 0.7,
            max_tokens: 512,
            cache: true,
            cache_handler: Some(cache_handler),
        }
    }

    /// Mimics [`LM::call`] without hitting a remote provider.
    ///
    /// The provided `prediction` becomes the assistant output and is inserted
    /// into the shared cache when caching is enabled.
    pub async fn call(
        &self,
        example: Example,
        messages: Chat,
        prediction: String,
    ) -> Result<LMResponse> {
        let mut full_chat = messages.clone();
        full_chat.push_message(Message::Assistant {
            content: prediction.clone(),
        });

        if self.cache
            && let Some(cache) = self.cache_handler.as_ref()
        {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            let cache_clone = cache.clone();
            let example_clone = example.clone();

            // Spawn the cache insert operation to avoid deadlock
            tokio::spawn(async move {
                let _ = cache_clone.lock().await.insert(example_clone, rx).await;
            });

            // Send the result to the cache
            tx.send(CallResult {
                prompt: messages.to_json().to_string(),
                prediction: Prediction::new(
                    HashMap::from([("prediction".to_string(), prediction.clone().into())]),
                    LmUsage::default(),
                ),
            })
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send to cache"))?;
        }

        Ok(LMResponse {
            output: Message::Assistant {
                content: prediction.clone(),
            },
            usage: LmUsage::default(),
            chat: full_chat,
            tool_calls: Vec::new(),
            tool_executions: Vec::new(),
        })
    }

    /// Returns cached entries just like [`LM::inspect_history`].
    pub async fn inspect_history(&self, n: usize) -> Vec<CallResult> {
        self.cache_handler
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_history(n)
            .await
            .unwrap()
    }
}
