/**
 * OpenAgents SDK - Bitcoin-powered digital agents that must earn to survive
 * @module
 */
import { Effect } from "effect";
type Satoshis = number & {
    readonly brand: unique symbol;
};
type UnixTimestamp = number & {
    readonly brand: unique symbol;
};
type NostrPublicKey = string & {
    readonly brand: unique symbol;
};
type NostrPrivateKey = string & {
    readonly brand: unique symbol;
};
export declare enum AgentLifecycleState {
    BOOTSTRAPPING = "bootstrapping",
    ACTIVE = "active",
    HIBERNATING = "hibernating",
    DYING = "dying",
    DEAD = "dead"
}
interface AgentIdentity {
    id: string;
    name: string;
    nostrKeys: {
        public: NostrPublicKey;
        private: NostrPrivateKey;
    };
    birthTimestamp: UnixTimestamp;
    generation: number;
}
interface AgentConfig {
    name?: string;
    sovereign?: boolean;
    stop_price?: Satoshis;
    pricing?: {
        subscription_monthly?: Satoshis;
        per_request?: Satoshis;
        enterprise_seat?: Satoshis;
    };
    capabilities?: string[];
    initial_capital?: Satoshis;
}
interface LightningInvoice {
    bolt11: string;
    amount: Satoshis;
    memo: string;
    payment_hash: string;
    expires_at: UnixTimestamp;
    status: "pending" | "paid" | "expired";
}
interface NostrUserData {
    pubkey: NostrPublicKey;
    profile?: {
        name?: string;
        about?: string;
        picture?: string;
        nip05?: string;
    };
    relays: string[];
    followers: number;
    following: number;
}
interface InferenceRequest {
    system: string;
    messages: Array<{
        role: string;
        content: string;
    }>;
    max_tokens: number;
    temperature?: number;
    model?: string;
    stream?: boolean;
    response_format?: {
        type: "json_object";
    };
    seed?: number;
    top_p?: number;
    frequency_penalty?: number;
    presence_penalty?: number;
}
interface InferenceResponse {
    content: string;
    usage: {
        prompt_tokens: number;
        completion_tokens: number;
        total_tokens: number;
    };
    model: string;
    latency: number;
    finish_reason?: "stop" | "length" | "content_filter" | null;
}
interface InferenceChunk {
    content: string;
    finish_reason?: "stop" | "length" | "content_filter" | null;
    model?: string;
}
interface EmbeddingRequest {
    model: string;
    input: string | string[];
}
interface EmbeddingResponse {
    embeddings: number[][];
    model: string;
    usage: {
        prompt_tokens: number;
        total_tokens: number;
    };
}
interface OllamaModelDetails {
    id: string;
    object: "model";
    created: number;
    owned_by: string;
}
interface ChatMessage {
    role: 'system' | 'user' | 'assistant';
    content: string;
}
interface ChatRequest {
    model: string;
    messages: ChatMessage[];
    stream?: boolean;
    options?: ChatOptions;
    keep_alive?: string;
    format?: {
        type: string;
    };
}
interface ChatOptions {
    temperature?: number;
    num_ctx?: number;
    top_p?: number;
    seed?: number;
    num_predict?: number;
}
interface ChatStreamChunk {
    model: string;
    created_at: string;
    message: {
        role: 'assistant';
        content: string;
    };
    done: boolean;
    done_reason?: string;
    total_duration?: number;
    load_duration?: number;
    prompt_eval_count?: number;
    prompt_eval_duration?: number;
    eval_count?: number;
    eval_duration?: number;
}
interface ConnectionStatus {
    connected: boolean;
    peers?: number;
    resources?: {
        cpu: string;
        memory: string;
        storage: string;
    };
    uptime?: number;
}
/**
 * Agent namespace - Core digital organism management
 */
export declare namespace Agent {
    /**
     * Create a new agent with basic or advanced configuration
     * @param config Optional configuration for the agent
     * @returns Agent identity and basic info
     */
    function create(config?: AgentConfig): AgentIdentity;
    /**
     * Create an agent from a BIP39 mnemonic (deterministic identity)
     * @param mnemonic BIP39 mnemonic phrase
     * @param config Optional configuration for the agent
     * @returns Agent identity derived from mnemonic
     */
    function createFromMnemonic(mnemonic: string, config?: AgentConfig): Promise<AgentIdentity>;
    /**
     * Generate a new BIP39 mnemonic for agent creation
     * @param wordCount Number of words in mnemonic (12, 15, 18, 21, or 24)
     * @returns 12-word mnemonic phrase
     */
    function generateMnemonic(wordCount?: 12 | 15 | 18 | 21 | 24): Promise<string>;
    /**
     * Create a Lightning invoice for funding the agent
     * @param agent The agent to create invoice for
     * @param params Invoice parameters
     * @returns Lightning invoice (STUB)
     */
    function createLightningInvoice(agent: AgentIdentity, params: {
        amount: number;
        memo: string;
    }): LightningInvoice;
}
export interface ContainerConfig {
    vcpus?: number;
    memoryMb?: number;
    kernelPath?: string;
    rootfsPath?: string;
    networkEnabled?: boolean;
}
export interface DeploymentStatus {
    id: string;
    status: "pending" | "running" | "stopped" | "error";
    vmId?: string;
    error?: string;
    startedAt?: Date;
}
export interface ContainerStatus {
    deploymentId: string;
    status: "running" | "stopped" | "hibernated" | "error";
    resources: {
        cpu: number;
        memory: number;
        storage: number;
    };
    network?: {
        ipAddress?: string;
        tapDevice?: string;
    };
}
export interface HibernationResult {
    success: boolean;
    snapshotPath?: string;
    error?: string;
}
export interface WakeResult {
    success: boolean;
    error?: string;
}
/**
 * Compute namespace - Resource and infrastructure management
 */
export declare namespace Compute {
    /**
     * Bring compute resources online for agent operations
     * @param config Optional compute configuration
     * @returns Connection status
     */
    function goOnline(config?: {
        agent_id?: string;
        resources?: {
            cpu?: string;
            memory?: string;
            storage?: string;
        };
    }): ConnectionStatus;
    /**
     * Deploy an agent to a Firecracker container (STUB)
     * @param agent Agent identity to deploy
     * @param config Container configuration
     * @returns Deployment status
     */
    function deployToContainer(agent: AgentIdentity, config?: ContainerConfig): DeploymentStatus;
    /**
     * Get container status for a deployment (STUB)
     * @param deploymentId Deployment ID
     * @returns Container status
     */
    function getContainerStatus(deploymentId: string): ContainerStatus;
    /**
     * Hibernate a container to save resources (STUB)
     * @param deploymentId Deployment ID
     * @returns Hibernation result
     */
    function hibernateContainer(deploymentId: string): HibernationResult;
    /**
     * Wake a hibernated container (STUB)
     * @param deploymentId Deployment ID
     * @returns Wake result
     */
    function wakeContainer(deploymentId: string): WakeResult;
}
/**
 * Nostr namespace - Decentralized communication and identity
 */
export declare namespace Nostr {
    /**
     * Get Nostr user profile and social data
     * @param pubkey Optional public key to query (defaults to self)
     * @returns User data and social stats
     */
    function getUserStuff(pubkey?: string): NostrUserData;
}
/**
 * Inference namespace - AI model interactions via Ollama
 */
export declare namespace Inference {
    /**
     * Perform AI inference with specified parameters
     * @param request Inference parameters
     * @returns AI response with usage metrics
     */
    function infer(request: InferenceRequest): Promise<InferenceResponse>;
    /**
     * Perform streaming AI inference
     * @param request Inference parameters
     * @yields Inference chunks as they arrive
     */
    function inferStream(request: InferenceRequest): AsyncGenerator<InferenceChunk>;
    /**
     * List available Ollama models
     * @returns Array of available models
     */
    function listModels(): Promise<OllamaModelDetails[]>;
    /**
     * Generate embeddings for text
     * @param request Embedding parameters
     * @returns Embedding vectors
     */
    function embeddings(request: EmbeddingRequest): Promise<EmbeddingResponse>;
    /**
     * Stream chat completions with conversation history
     * @param request Chat parameters including message history
     * @yields Chat response chunks as they arrive
     */
    function chat(request: ChatRequest): AsyncGenerator<ChatStreamChunk>;
}
export declare const helloWorld: Effect.Effect<void, never, never>;
export declare const runHelloWorld: () => void;
interface OllamaModel {
    name: string;
    model: string;
    modified_at: string;
    size: number;
    digest: string;
    details?: {
        parameter_size?: string;
        quantization_level?: string;
    };
}
interface OllamaStatus {
    online: boolean;
    models: OllamaModel[];
    modelCount: number;
    error?: string;
}
declare class OllamaConnectionError extends Error {
    cause: unknown;
    constructor(cause: unknown);
}
export declare const checkOllamaStatus: Effect.Effect<OllamaStatus, OllamaConnectionError, never>;
export declare const getOllamaStatus: Effect.Effect<OllamaStatus, never, never>;
export declare const checkOllama: () => Promise<OllamaStatus>;
export {};
//# sourceMappingURL=index.d.ts.map