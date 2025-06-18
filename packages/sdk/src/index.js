/**
 * OpenAgents SDK - Bitcoin-powered digital agents that must earn to survive
 * @module
 */
import { Effect, Console } from "effect";
// Helper to create branded types
const asSatoshis = (n) => n;
const asTimestamp = (n) => n;
const asNostrPubKey = (s) => s;
const asNostrPrivKey = (s) => s;
// Agent lifecycle states (exported for external use)
export var AgentLifecycleState;
(function (AgentLifecycleState) {
    AgentLifecycleState["BOOTSTRAPPING"] = "bootstrapping";
    AgentLifecycleState["ACTIVE"] = "active";
    AgentLifecycleState["HIBERNATING"] = "hibernating";
    AgentLifecycleState["DYING"] = "dying";
    AgentLifecycleState["DEAD"] = "dead";
})(AgentLifecycleState || (AgentLifecycleState = {}));
/**
 * Agent namespace - Core digital organism management
 */
export var Agent;
(function (Agent) {
    /**
     * Create a new agent with basic or advanced configuration
     * @param config Optional configuration for the agent
     * @returns Agent identity and basic info
     */
    function create(config = {}) {
        // Generate deterministic ID from timestamp and random
        const id = `agent_${Date.now()}_${Math.random().toString(36).substring(2, 10)}`;
        // Generate proper Nostr keys using NIP-06 (deterministic from mnemonic)
        // For now using random keys, but this will be enhanced with proper mnemonic generation
        const privateKey = Array.from({ length: 64 }, () => Math.floor(Math.random() * 16).toString(16)).join('');
        const publicKey = `npub${Array.from({ length: 58 }, () => Math.floor(Math.random() * 36).toString(36)).join('')}`;
        const agent = {
            id,
            name: config.name || `Agent-${id.slice(-8)}`,
            nostrKeys: {
                public: asNostrPubKey(publicKey),
                private: asNostrPrivKey(privateKey)
            },
            birthTimestamp: asTimestamp(Date.now()),
            generation: 0
        };
        return agent;
    }
    Agent.create = create;
    /**
     * Create an agent from a BIP39 mnemonic (deterministic identity)
     * @param mnemonic BIP39 mnemonic phrase
     * @param config Optional configuration for the agent
     * @returns Agent identity derived from mnemonic
     */
    async function createFromMnemonic(mnemonic, config = {}) {
        // TODO: Re-enable after build order is fixed
        // Use actual NIP-06 service for proper key derivation
        // const keys = await Effect.gen(function*() {
        //   const nip06 = yield* NostrLib.Nip06Service.Nip06Service
        //   return yield* nip06.deriveAllKeys(mnemonic as NostrLib.Schema.Mnemonic)
        // }).pipe(
        //   Effect.provide(
        //     NostrLib.Nip06Service.Nip06ServiceLive.pipe(
        //       Layer.provide(NostrLib.CryptoService.CryptoServiceLive)
        //     )
        //   ),
        //   Effect.runPromise
        // )
        // STUB: Generate random keys until Nostr service is available
        const privateKey = Array.from({ length: 64 }, () => Math.floor(Math.random() * 16).toString(16)).join('');
        const publicKey = `npub${Array.from({ length: 58 }, () => Math.floor(Math.random() * 36).toString(36)).join('')}`;
        // Create deterministic ID from the public key
        const id = `agent_${publicKey.slice(-12)}`;
        const agent = {
            id,
            name: config.name || `Agent-${publicKey.slice(-8)}`,
            nostrKeys: {
                public: asNostrPubKey(publicKey),
                private: asNostrPrivKey(privateKey)
            },
            birthTimestamp: asTimestamp(Date.now()),
            generation: 0
        };
        return agent;
    }
    Agent.createFromMnemonic = createFromMnemonic;
    /**
     * Generate a new BIP39 mnemonic for agent creation
     * @param wordCount Number of words in mnemonic (12, 15, 18, 21, or 24)
     * @returns 12-word mnemonic phrase
     */
    async function generateMnemonic(wordCount = 12) {
        // TODO: Re-enable after build order is fixed
        // const mnemonic = await Effect.gen(function*() {
        //   const nip06 = yield* NostrLib.Nip06Service.Nip06Service
        //   return yield* nip06.generateMnemonic(wordCount)
        // }).pipe(
        //   Effect.provide(
        //     NostrLib.Nip06Service.Nip06ServiceLive.pipe(
        //       Layer.provide(NostrLib.CryptoService.CryptoServiceLive)
        //     )
        //   ),
        //   Effect.runPromise
        // )
        // STUB: Generate a dummy mnemonic until Nostr service is available
        const words = ['abandon', 'ability', 'able', 'about', 'above', 'absent', 'absorb', 'abstract', 'absurd', 'abuse', 'access', 'accident'];
        const mnemonic = Array.from({ length: wordCount }, () => words[Math.floor(Math.random() * words.length)]).join(' ');
        return mnemonic;
    }
    Agent.generateMnemonic = generateMnemonic;
    /**
     * Create a Lightning invoice for funding the agent
     * @param agent The agent to create invoice for
     * @param params Invoice parameters
     * @returns Lightning invoice (STUB)
     */
    function createLightningInvoice(agent, params) {
        const invoice = {
            bolt11: `lnbc${params.amount}u1p...stub`, // STUB bolt11 format
            amount: asSatoshis(params.amount),
            memo: params.memo,
            payment_hash: Array.from({ length: 64 }, () => Math.floor(Math.random() * 16).toString(16)).join(''),
            expires_at: asTimestamp(Date.now() + 3600000), // 1 hour
            status: "pending"
        };
        return invoice;
    }
    Agent.createLightningInvoice = createLightningInvoice;
})(Agent || (Agent = {}));
/**
 * Compute namespace - Resource and infrastructure management
 */
export var Compute;
(function (Compute) {
    /**
     * Bring compute resources online for agent operations
     * @param config Optional compute configuration
     * @returns Connection status
     */
    function goOnline(config = {}) {
        const status = {
            connected: true,
            peers: Math.floor(Math.random() * 50) + 10, // Simulate 10-60 peers
            resources: {
                cpu: config.resources?.cpu || "2 cores",
                memory: config.resources?.memory || "4GB",
                storage: config.resources?.storage || "10GB"
            },
            uptime: Date.now()
        };
        return status;
    }
    Compute.goOnline = goOnline;
    /**
     * Deploy an agent to a Firecracker container (STUB)
     * @param agent Agent identity to deploy
     * @param config Container configuration
     * @returns Deployment status
     */
    function deployToContainer(agent, config = {}) {
        const deploymentId = `deployment_${agent.id}_${Date.now()}`;
        // STUB: In real implementation, this would call the container service
        const status = {
            id: deploymentId,
            status: "pending",
            vmId: `vm_${deploymentId}`,
            startedAt: new Date()
        };
        console.log(`[STUB] Deploying agent ${agent.id} to container with config:`, config);
        return status;
    }
    Compute.deployToContainer = deployToContainer;
    /**
     * Get container status for a deployment (STUB)
     * @param deploymentId Deployment ID
     * @returns Container status
     */
    function getContainerStatus(deploymentId) {
        // STUB: In real implementation, this would query the container service
        const status = {
            deploymentId,
            status: "running",
            resources: {
                cpu: 1,
                memory: 256,
                storage: 1024
            },
            network: {
                ipAddress: "10.0.0.2",
                tapDevice: "tap0"
            }
        };
        console.log(`[STUB] Getting container status for deployment ${deploymentId}`);
        return status;
    }
    Compute.getContainerStatus = getContainerStatus;
    /**
     * Hibernate a container to save resources (STUB)
     * @param deploymentId Deployment ID
     * @returns Hibernation result
     */
    function hibernateContainer(deploymentId) {
        // STUB: In real implementation, this would use CRIU via container service
        const result = {
            success: true,
            snapshotPath: `/var/lib/openagents/snapshots/${deploymentId}.img`
        };
        console.log(`[STUB] Hibernating container for deployment ${deploymentId}`);
        return result;
    }
    Compute.hibernateContainer = hibernateContainer;
    /**
     * Wake a hibernated container (STUB)
     * @param deploymentId Deployment ID
     * @returns Wake result
     */
    function wakeContainer(deploymentId) {
        // STUB: In real implementation, this would restore from CRIU snapshot
        const result = {
            success: true
        };
        console.log(`[STUB] Waking container for deployment ${deploymentId}`);
        return result;
    }
    Compute.wakeContainer = wakeContainer;
})(Compute || (Compute = {}));
/**
 * Nostr namespace - Decentralized communication and identity
 */
export var Nostr;
(function (Nostr) {
    /**
     * Get Nostr user profile and social data
     * @param pubkey Optional public key to query (defaults to self)
     * @returns User data and social stats
     */
    function getUserStuff(pubkey) {
        const userData = {
            pubkey: asNostrPubKey(pubkey || `npub${Array.from({ length: 58 }, () => Math.floor(Math.random() * 36).toString(36)).join('')}`),
            profile: {
                name: "Agent User",
                about: "Digital agent on the Nostr network",
                picture: "https://openagents.com/avatar.png",
                nip05: "agent@openagents.com"
            },
            relays: [
                "wss://relay.damus.io",
                "wss://relay.nostr.band",
                "wss://nos.lol"
            ],
            followers: Math.floor(Math.random() * 1000),
            following: Math.floor(Math.random() * 500)
        };
        return userData;
    }
    Nostr.getUserStuff = getUserStuff;
})(Nostr || (Nostr = {}));
/**
 * Inference namespace - AI model interactions via Ollama
 */
export var Inference;
(function (Inference) {
    const OLLAMA_BASE_URL = "http://localhost:11434";
    const OLLAMA_OPENAI_URL = "http://localhost:11434/v1";
    const OLLAMA_API_KEY = "ollama"; // Required but ignored by Ollama
    let useOpenAIMode = false; // Will be determined dynamically
    /**
     * Check if Ollama is available and determine which API to use
     */
    async function isOllamaAvailable() {
        // First try OpenAI compatibility endpoint
        try {
            const response = await fetch(`${OLLAMA_OPENAI_URL}/models`, {
                headers: { "Authorization": `Bearer ${OLLAMA_API_KEY}` }
            });
            if (response.ok) {
                useOpenAIMode = true;
                return true;
            }
        }
        catch { }
        // Fall back to native API
        try {
            const response = await fetch(`${OLLAMA_BASE_URL}/api/tags`);
            if (response.ok) {
                useOpenAIMode = false;
                return true;
            }
        }
        catch { }
        return false;
    }
    /**
     * Perform AI inference with specified parameters
     * @param request Inference parameters
     * @returns AI response with usage metrics
     */
    async function infer(request) {
        const startTime = Date.now();
        // Check if Ollama is available
        const ollamaAvailable = await isOllamaAvailable();
        if (!ollamaAvailable) {
            throw new Error("Ollama is not available. Please ensure Ollama is running at http://localhost:11434");
        }
        // Prepare messages with system prompt
        const messages = [
            { role: "system", content: request.system },
            ...request.messages
        ];
        try {
            // If no model specified, use default
            let modelToUse = request.model || "llama3.2";
            if (useOpenAIMode) {
                // OpenAI compatibility mode
                const response = await fetch(`${OLLAMA_OPENAI_URL}/chat/completions`, {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json",
                        "Authorization": `Bearer ${OLLAMA_API_KEY}`
                    },
                    body: JSON.stringify({
                        model: modelToUse,
                        messages,
                        max_tokens: request.max_tokens,
                        temperature: request.temperature,
                        stream: false,
                        response_format: request.response_format,
                        seed: request.seed,
                        top_p: request.top_p,
                        frequency_penalty: request.frequency_penalty,
                        presence_penalty: request.presence_penalty
                    })
                });
                if (!response.ok) {
                    const errorText = await response.text();
                    console.error(`Ollama OpenAI API error (${response.status}):`, errorText);
                    throw new Error(`Ollama API error: ${response.statusText} - ${errorText}`);
                }
                const data = await response.json();
                const endTime = Date.now();
                const result = {
                    content: data.choices[0].message.content,
                    usage: data.usage,
                    model: data.model,
                    latency: endTime - startTime,
                    finish_reason: data.choices[0].finish_reason
                };
                return result;
            }
            else {
                // Native Ollama API mode
                const prompt = messages.map(msg => {
                    if (msg.role === "system")
                        return `System: ${msg.content}`;
                    if (msg.role === "user")
                        return `Human: ${msg.content}`;
                    if (msg.role === "assistant")
                        return `Assistant: ${msg.content}`;
                    return msg.content;
                }).join("\n\n") + "\n\nAssistant:";
                const response = await fetch(`${OLLAMA_BASE_URL}/api/generate`, {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json"
                    },
                    body: JSON.stringify({
                        model: modelToUse,
                        prompt,
                        options: {
                            num_predict: request.max_tokens,
                            temperature: request.temperature,
                            top_p: request.top_p,
                            seed: request.seed
                        },
                        stream: false,
                        format: request.response_format?.type === "json_object" ? "json" : undefined
                    })
                });
                if (!response.ok) {
                    const errorText = await response.text();
                    console.error(`Ollama native API error (${response.status}):`, errorText);
                    throw new Error(`Ollama API error: ${response.statusText} - ${errorText}`);
                }
                const data = await response.json();
                const endTime = Date.now();
                // Calculate approximate token counts
                const promptTokens = Math.floor(prompt.length / 4);
                const completionTokens = Math.floor(data.response.length / 4);
                const result = {
                    content: data.response,
                    usage: {
                        prompt_tokens: promptTokens,
                        completion_tokens: completionTokens,
                        total_tokens: promptTokens + completionTokens
                    },
                    model: data.model,
                    latency: endTime - startTime,
                    finish_reason: data.done ? "stop" : "length"
                };
                return result;
            }
        }
        catch (error) {
            console.error("Ollama inference error:", error);
            throw error;
        }
    }
    Inference.infer = infer;
    /**
     * Perform streaming AI inference
     * @param request Inference parameters
     * @yields Inference chunks as they arrive
     */
    async function* inferStream(request) {
        const ollamaAvailable = await isOllamaAvailable();
        if (!ollamaAvailable) {
            throw new Error("Ollama is not available. Please ensure Ollama is running at http://localhost:11434");
        }
        const messages = [
            { role: "system", content: request.system },
            ...request.messages
        ];
        // If no model specified, use default
        let modelToUse = request.model || "llama3.2";
        try {
            if (useOpenAIMode) {
                // OpenAI compatibility mode streaming
                const response = await fetch(`${OLLAMA_OPENAI_URL}/chat/completions`, {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json",
                        "Authorization": `Bearer ${OLLAMA_API_KEY}`
                    },
                    body: JSON.stringify({
                        model: modelToUse,
                        messages,
                        max_tokens: request.max_tokens,
                        temperature: request.temperature,
                        stream: true,
                        response_format: request.response_format,
                        seed: request.seed,
                        top_p: request.top_p,
                        frequency_penalty: request.frequency_penalty,
                        presence_penalty: request.presence_penalty
                    })
                });
                if (!response.ok) {
                    const errorText = await response.text();
                    console.error(`Ollama OpenAI streaming error (${response.status}):`, errorText);
                    throw new Error(`Ollama API error: ${response.statusText} - ${errorText}`);
                }
                const reader = response.body?.getReader();
                if (!reader)
                    throw new Error("No response body");
                const decoder = new TextDecoder();
                let buffer = "";
                while (true) {
                    const { done, value } = await reader.read();
                    if (done)
                        break;
                    buffer += decoder.decode(value, { stream: true });
                    const lines = buffer.split("\n");
                    buffer = lines.pop() || "";
                    for (const line of lines) {
                        if (line.startsWith("data: ")) {
                            const data = line.slice(6);
                            if (data === "[DONE]")
                                return;
                            try {
                                const chunk = JSON.parse(data);
                                const content = chunk.choices[0]?.delta?.content || "";
                                const finish_reason = chunk.choices[0]?.finish_reason || null;
                                yield { content, finish_reason, model: chunk.model };
                            }
                            catch {
                                // Skip invalid JSON
                            }
                        }
                    }
                }
            }
            else {
                // Native Ollama API streaming
                const prompt = messages.map(msg => {
                    if (msg.role === "system")
                        return `System: ${msg.content}`;
                    if (msg.role === "user")
                        return `Human: ${msg.content}`;
                    if (msg.role === "assistant")
                        return `Assistant: ${msg.content}`;
                    return msg.content;
                }).join("\n\n") + "\n\nAssistant:";
                const response = await fetch(`${OLLAMA_BASE_URL}/api/generate`, {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json"
                    },
                    body: JSON.stringify({
                        model: modelToUse,
                        prompt,
                        options: {
                            num_predict: request.max_tokens,
                            temperature: request.temperature,
                            top_p: request.top_p,
                            seed: request.seed
                        },
                        stream: true,
                        format: request.response_format?.type === "json_object" ? "json" : undefined
                    })
                });
                if (!response.ok) {
                    const errorText = await response.text();
                    console.error(`Ollama native streaming error (${response.status}):`, errorText);
                    throw new Error(`Ollama API error: ${response.statusText} - ${errorText}`);
                }
                const reader = response.body?.getReader();
                if (!reader)
                    throw new Error("No response body");
                const decoder = new TextDecoder();
                let buffer = "";
                while (true) {
                    const { done, value } = await reader.read();
                    if (done)
                        break;
                    buffer += decoder.decode(value, { stream: true });
                    const lines = buffer.split("\n");
                    buffer = lines.pop() || "";
                    for (const line of lines) {
                        if (line.trim()) {
                            try {
                                const chunk = JSON.parse(line);
                                if (chunk.response) {
                                    yield {
                                        content: chunk.response,
                                        finish_reason: chunk.done ? "stop" : null,
                                        model: chunk.model
                                    };
                                }
                                if (chunk.done) {
                                    return;
                                }
                            }
                            catch {
                                // Skip invalid JSON
                            }
                        }
                    }
                }
            }
        }
        catch (error) {
            console.error("Ollama streaming error:", error);
            throw error;
        }
    }
    Inference.inferStream = inferStream;
    /**
     * List available Ollama models
     * @returns Array of available models
     */
    async function listModels() {
        try {
            // Try OpenAI compatibility first, then fall back to native API
            let response;
            try {
                response = await fetch(`${OLLAMA_OPENAI_URL}/models`, {
                    headers: { "Authorization": `Bearer ${OLLAMA_API_KEY}` }
                });
                if (response.ok) {
                    const data = await response.json();
                    useOpenAIMode = true;
                    return data.data || [];
                }
            }
            catch { }
            // Fall back to native API
            response = await fetch(`${OLLAMA_BASE_URL}/api/tags`);
            if (!response.ok) {
                throw new Error(`Failed to list models: ${response.statusText}`);
            }
            const data = await response.json();
            useOpenAIMode = false;
            // Convert Ollama format to OpenAI-like format
            return (data.models || []).map((model) => ({
                id: model.name,
                object: "model",
                created: Math.floor(new Date(model.modified_at).getTime() / 1000),
                owned_by: "ollama"
            }));
        }
        catch (error) {
            console.error("Failed to list Ollama models:", error);
            return [];
        }
    }
    Inference.listModels = listModels;
    /**
     * Generate embeddings for text
     * @param request Embedding parameters
     * @returns Embedding vectors
     */
    async function embeddings(request) {
        try {
            // Ollama uses /api/embeddings endpoint
            const inputs = Array.isArray(request.input) ? request.input : [request.input];
            const embeddings = [];
            // Process each input separately (Ollama doesn't support batch embeddings)
            for (const input of inputs) {
                const response = await fetch(`${OLLAMA_BASE_URL}/api/embeddings`, {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json"
                    },
                    body: JSON.stringify({
                        model: request.model,
                        prompt: input
                    })
                });
                if (!response.ok) {
                    throw new Error(`Failed to generate embeddings: ${response.statusText}`);
                }
                const data = await response.json();
                embeddings.push(data.embedding);
            }
            // Calculate token usage (approximate)
            const totalTokens = inputs.reduce((sum, input) => sum + Math.floor(input.length / 4), 0);
            return {
                embeddings,
                model: request.model,
                usage: {
                    prompt_tokens: totalTokens,
                    total_tokens: totalTokens
                }
            };
        }
        catch (error) {
            console.error("Failed to generate embeddings:", error);
            throw error;
        }
    }
    Inference.embeddings = embeddings;
    /**
     * Stream chat completions with conversation history
     * @param request Chat parameters including message history
     * @yields Chat response chunks as they arrive
     */
    async function* chat(request) {
        const ollamaAvailable = await isOllamaAvailable();
        if (!ollamaAvailable) {
            throw new Error("Ollama is not available. Please ensure Ollama is running at http://localhost:11434");
        }
        // Default to streaming unless explicitly disabled
        const shouldStream = request.stream !== false;
        try {
            if (useOpenAIMode) {
                // OpenAI compatibility mode
                const response = await fetch(`${OLLAMA_OPENAI_URL}/chat/completions`, {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json",
                        "Authorization": `Bearer ${OLLAMA_API_KEY}`
                    },
                    body: JSON.stringify({
                        model: request.model,
                        messages: request.messages,
                        stream: shouldStream,
                        temperature: request.options?.temperature,
                        max_tokens: request.options?.num_predict,
                        top_p: request.options?.top_p,
                        seed: request.options?.seed,
                        response_format: request.format ? { type: "json_object" } : undefined
                    })
                });
                if (!response.ok) {
                    const errorText = await response.text();
                    console.error(`Ollama chat error (${response.status}):`, errorText);
                    throw new Error(`Ollama API error: ${response.statusText} - ${errorText}`);
                }
                if (!shouldStream) {
                    // Non-streaming response
                    const data = await response.json();
                    yield {
                        model: data.model,
                        created_at: new Date().toISOString(),
                        message: {
                            role: 'assistant',
                            content: data.choices[0].message.content
                        },
                        done: true,
                        total_duration: 0,
                        eval_count: data.usage?.completion_tokens || 0,
                        prompt_eval_count: data.usage?.prompt_tokens || 0
                    };
                    return;
                }
                // Streaming response
                const reader = response.body?.getReader();
                if (!reader)
                    throw new Error("No response body");
                const decoder = new TextDecoder();
                let buffer = "";
                while (true) {
                    const { done, value } = await reader.read();
                    if (done)
                        break;
                    buffer += decoder.decode(value, { stream: true });
                    const lines = buffer.split("\n");
                    buffer = lines.pop() || "";
                    for (const line of lines) {
                        if (line.startsWith("data: ")) {
                            const data = line.slice(6);
                            if (data === "[DONE]")
                                return;
                            try {
                                const chunk = JSON.parse(data);
                                const content = chunk.choices[0]?.delta?.content || "";
                                const isComplete = chunk.choices[0]?.finish_reason !== null;
                                yield {
                                    model: chunk.model || request.model,
                                    created_at: new Date().toISOString(),
                                    message: {
                                        role: 'assistant',
                                        content
                                    },
                                    done: isComplete,
                                    done_reason: chunk.choices[0]?.finish_reason
                                };
                            }
                            catch {
                                // Skip invalid JSON
                            }
                        }
                    }
                }
            }
            else {
                // Native Ollama API mode
                const response = await fetch(`${OLLAMA_BASE_URL}/api/chat`, {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json"
                    },
                    body: JSON.stringify({
                        model: request.model,
                        messages: request.messages,
                        stream: shouldStream,
                        options: {
                            temperature: request.options?.temperature,
                            num_ctx: request.options?.num_ctx,
                            num_predict: request.options?.num_predict,
                            top_p: request.options?.top_p,
                            seed: request.options?.seed
                        },
                        keep_alive: request.keep_alive,
                        format: request.format?.type === "json_object" ? "json" : undefined
                    })
                });
                if (!response.ok) {
                    const errorText = await response.text();
                    console.error(`Ollama chat error (${response.status}):`, errorText);
                    throw new Error(`Ollama API error: ${response.statusText} - ${errorText}`);
                }
                if (!shouldStream) {
                    // Non-streaming response
                    const data = await response.json();
                    yield data;
                    return;
                }
                // Streaming response
                const reader = response.body?.getReader();
                if (!reader)
                    throw new Error("No response body");
                const decoder = new TextDecoder();
                let buffer = "";
                while (true) {
                    const { done, value } = await reader.read();
                    if (done)
                        break;
                    buffer += decoder.decode(value, { stream: true });
                    const lines = buffer.split("\n");
                    buffer = lines.pop() || "";
                    for (const line of lines) {
                        if (line.trim()) {
                            try {
                                const chunk = JSON.parse(line);
                                yield chunk;
                                if (chunk.done) {
                                    return;
                                }
                            }
                            catch {
                                // Skip invalid JSON
                            }
                        }
                    }
                }
            }
        }
        catch (error) {
            console.error("Ollama chat error:", error);
            throw error;
        }
    }
    Inference.chat = chat;
})(Inference || (Inference = {}));
// Legacy exports for backward compatibility
export const helloWorld = Console.log("Hello from OpenAgents SDK!");
export const runHelloWorld = () => Effect.runSync(helloWorld);
class OllamaConnectionError extends Error {
    cause;
    constructor(cause) {
        super("Failed to connect to Ollama");
        this.cause = cause;
        this.name = "OllamaConnectionError";
    }
}
const OLLAMA_DEFAULT_PORT = 11434;
const getOllamaBaseUrl = () => {
    return `http://localhost:${OLLAMA_DEFAULT_PORT}`;
};
export const checkOllamaStatus = Effect.tryPromise({
    try: async () => {
        const baseUrl = getOllamaBaseUrl();
        const response = await fetch(`${baseUrl}/api/tags`, {
            method: 'GET',
            headers: { 'Accept': 'application/json' },
            signal: AbortSignal.timeout(3000)
        });
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        const data = await response.json();
        return {
            online: true,
            models: data.models || [],
            modelCount: (data.models || []).length
        };
    },
    catch: (error) => new OllamaConnectionError(error)
});
const checkOllamaRootEndpoint = Effect.tryPromise({
    try: async () => {
        const response = await fetch(`${getOllamaBaseUrl()}/`, {
            method: 'GET',
            signal: AbortSignal.timeout(3000)
        });
        return {
            online: response.ok || response.status < 500,
            models: [],
            modelCount: 0
        };
    },
    catch: (error) => new OllamaConnectionError(error)
});
export const getOllamaStatus = Effect.orElse(checkOllamaStatus, () => checkOllamaRootEndpoint).pipe(Effect.catchAll(() => Effect.succeed({
    online: false,
    models: [],
    modelCount: 0,
    error: "Cannot connect to Ollama"
})));
export const checkOllama = () => Effect.runPromise(getOllamaStatus);
//# sourceMappingURL=index.js.map