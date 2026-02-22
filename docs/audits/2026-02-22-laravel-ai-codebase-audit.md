# Laravel AI Codebase Audit (for OpenAgents Harvest)

Date: 2026-02-22
Audited repository: `/Users/christopherdavid/code/laravel-ai`
Audit target in OpenAgents: `docs/audits/2026-02-22-laravel-ai-codebase-audit.md`

## 1) Executive Summary

`laravel/ai` is a Laravel package (not an application) that provides a unified AI SDK over many providers (OpenAI, Anthropic, Gemini, Groq, Mistral, etc.) with these major product surfaces:

- Agent prompting (sync, streaming SSE, queue, broadcast)
- Structured output and tool calling
- Audio generation (TTS) and transcription (STT)
- Embeddings and reranking
- File upload/retrieval and vector stores
- Conversation memory persistence (DB-backed)
- Fake gateways and assertion APIs for tests

Core architecture is capability-oriented:

- Public API layer (`Audio`, `Image`, `Embeddings`, `Reranking`, `Files`, `Stores`, `Transcription`, agent helpers)
- Provider abstraction layer (`Contracts/Providers/*`, `Providers/*`)
- Gateway layer (`Gateway/*`, mostly Prism-backed for text/image/audio/transcription/embeddings)
- Transport normalization layer (response/value objects + streaming events)
- Laravel integration layer (ServiceProvider, container, queue, events, storage, DB)

For OpenAgents, the highest-value harvest is the design pattern set (capability interfaces, failover semantics, stream event normalization, fake/test harness strategy), not a direct code transplant.

## 2) Repo Shape and Size

### Totals

- Non-`.git` files: **321**
- PHP files: **321**
- `src/` PHP files: **278**
- `tests/` files: **43**

### Top-level folders/files

- `src/`: package implementation
- `tests/`: feature/unit tests + fixtures
- `config/ai.php`: provider/default/caching config
- `database/migrations/`: conversation-memory schema
- `stubs/`: artisan-generated class templates
- `resources/boost/`: AI assistant guidance/skill metadata
- `functions.php`: global helpers (`agent()`, `pipeline()`, ULID, fake JSON data)
- `composer.json`: package dependencies/autoload/service provider registration

### `src/` module distribution (file count)

- `Providers` 37
- `Responses` 35
- `Contracts` 34
- `Gateway` 31
- `Events` 28
- `Files` 21
- `Streaming` 14
- `Prompts` 11
- `Concerns` 10
- `Attributes` 8
- `Jobs` 7
- `PendingResponses` 5
- `Messages` 5
- `Exceptions` 4
- `Console` 4
- `Tools` 2
- `_root` 18
- `Storage` 1
- `Migrations` 1
- `Middleware` 1
- `Enums` 1

## 3) Architectural Boundaries

## 3.1 Public API / entry surface

Primary entry classes at `src/` root:

- `Ai.php` facade over `AiManager`
- `Promptable.php` trait for agent classes
- Modal facades: `Audio.php`, `Embeddings.php`, `Image.php`, `Reranking.php`, `Files.php`, `Stores.php`, `Transcription.php`
- Agent shortcuts: `AnonymousAgent.php`, `StructuredAnonymousAgent.php`, `functions.php` `agent()`

Boundary role:

- Keeps app code provider-agnostic.
- Captures call parameters into prompt objects.
- Delegates to provider capability methods.

## 3.2 Provider capability boundary

Contracts in `src/Contracts/Providers/*` define capability-specific interfaces:

- `TextProvider`, `EmbeddingProvider`, `ImageProvider`, `AudioProvider`, `TranscriptionProvider`, `RerankingProvider`, `FileProvider`, `StoreProvider`
- Optional provider-tool capability interfaces: `SupportsFileSearch`, `SupportsWebFetch`, `SupportsWebSearch`

Providers in `src/Providers/*Provider.php` compose behavior through traits (`GeneratesText`, `StreamsText`, `ManagesFiles`, etc.).

Boundary role:

- Normalizes feature calls across vendors.
- Vendor-specific model defaults/options live in provider classes.

## 3.3 Gateway boundary

- Generic contracts: `src/Contracts/Gateway/*`
- Prism-backed multimodal gateway: `src/Gateway/Prism/PrismGateway.php`
- Direct HTTP gateways for non-Prism surfaces:
  - File/store: `OpenAiFileGateway`, `OpenAiStoreGateway`, `AnthropicFileGateway`, `GeminiFileGateway`, `GeminiStoreGateway`
  - Reranking/embeddings: `CohereGateway`, `JinaGateway`, `VoyageAiGateway`
  - xAI images: `XaiImageGateway`
  - ElevenLabs audio/transcription: `ElevenLabsGateway`

Boundary role:

- Converts normalized SDK request objects into vendor-specific HTTP/Prism calls.
- Converts vendor responses into SDK response/value objects.

## 3.4 Tooling boundary

- Agent tool contract: `src/Contracts/Tool.php`
- Tool input wrapper: `src/Tools/Request.php`
- Provider-tool definitions: `src/Providers/Tools/*`
- Prism tool bridge/conversions: `src/Gateway/Prism/PrismTool.php`, `AddsToolsToPrismRequests.php`

Boundary role:

- Supports both local callable tools and provider-native tools (web/file search/fetch).

## 3.5 Streaming/event boundary

- Stream event model: `src/Streaming/Events/*`
- Prism stream conversion: `src/Gateway/Prism/PrismStreamEvent.php`
- Stream response wrapper: `src/Responses/StreamableAgentResponse.php`
- Optional Vercel protocol conversion: `src/Responses/Concerns/CanStreamUsingVercelProtocol.php`

Boundary role:

- Converts provider event streams into a consistent event schema.
- Exposes SSE and protocol adapters.

## 3.6 Persistence boundary (conversation memory)

- Contract: `src/Contracts/ConversationStore.php`
- Default impl: `src/Storage/DatabaseConversationStore.php`
- Middleware: `src/Middleware/RememberConversation.php`
- Agent trait: `src/Concerns/RemembersConversations.php`
- Migration: `database/migrations/2026_01_11_000001_create_agent_conversations_table.php`

Boundary role:

- Persists user/assistant messages and conversation metadata out-of-band from providers.

## 3.7 Laravel platform coupling boundary

Strong framework coupling points:

- Service container and facade (`AiServiceProvider`, `Ai`)
- Queue jobs and pending dispatch wrappers
- Event dispatcher and broadcast
- HTTP client facade
- DB and Storage facades
- Collection/Stringable macros in provider boot

This is the main adaptation cost for any non-Laravel runtime.

## 4) Provider and Capability Matrix

Provider classes and implemented capabilities:

- `AnthropicProvider`: text, streaming, files, web fetch/search provider tools
- `AzureOpenAiProvider`: text, streaming, embeddings
- `CohereProvider`: embeddings, reranking
- `DeepSeekProvider`: text, streaming
- `ElevenLabsProvider`: audio, transcription
- `GeminiProvider`: text, streaming, embeddings, images, files, stores, file-search + web tools
- `GroqProvider`: text, streaming
- `JinaProvider`: embeddings, reranking
- `MistralProvider`: text, streaming, embeddings, transcription
- `OllamaProvider`: text, streaming, embeddings
- `OpenAiProvider`: text, streaming, embeddings, images, audio, transcription, files, stores, file-search/web-search
- `OpenRouterProvider`: text, streaming
- `VoyageAiProvider`: embeddings, reranking
- `XaiProvider`: text, streaming, images

Notable default model assumptions (examples):

- OpenAI: `gpt-5.2`, `gpt-5-nano`, `gpt-5.2-pro`, `gpt-image-1.5`, `gpt-4o-mini-tts`, `gpt-4o-transcribe-diarize`, `text-embedding-3-small`
- Anthropic: `claude-sonnet-4-6`, `claude-haiku-4-5-20251001`, `claude-opus-4-6`
- Gemini: `gemini-3-flash-preview`, `gemini-3-pro-preview`, `gemini-3-pro-image-preview`, `gemini-embedding-001`
- Groq: `openai/gpt-oss-20b` and `openai/gpt-oss-120b` defaults

## 5) Data Models and Domain Objects

## 5.1 Database schema models

From `database/migrations/2026_01_11_000001_create_agent_conversations_table.php`:

- `agent_conversations`
  - `id` (string UUID-like)
  - `user_id` (nullable foreign id)
  - `title`
  - timestamps
  - index on `user_id`, `updated_at`
- `agent_conversation_messages`
  - `id`
  - `conversation_id`
  - `user_id`
  - `agent` (class name)
  - `role`
  - `content`
  - `attachments` (json text)
  - `tool_calls` (json text)
  - `tool_results` (json text)
  - `usage` (json text)
  - `meta` (json text)
  - timestamps
  - composite and user indexes

No Eloquent models are defined; DB is accessed via query builder in `DatabaseConversationStore`.

## 5.2 Message models

`src/Messages/*`:

- `Message` (role + content)
- `UserMessage` (attachments)
- `AssistantMessage` (tool calls)
- `ToolResultMessage` (tool results)
- `MessageRole` enum

## 5.3 Prompt models

`src/Prompts/*` and `src/PendingResponses/*` encode request intent/options for each modality.

Examples:

- `AgentPrompt` (prompt text, attachments, timeout, provider/model)
- `ImagePrompt` (size, quality, attachments)
- `TranscriptionPrompt` (language, diarize, timeout)
- pending builders (`PendingImageGeneration`, etc.) for fluent option APIs and queueing

## 5.4 Response/value models

`src/Responses/*` and `src/Responses/Data/*`:

- `TextResponse`, `AgentResponse`, `StructuredAgentResponse`, `StreamedAgentResponse`
- `ImageResponse`, `AudioResponse`, `TranscriptionResponse`, `EmbeddingsResponse`, `RerankingResponse`
- Value objects: `Meta`, `Usage`, `ToolCall`, `ToolResult`, `Step`, `StructuredStep`, `RankedDocument`, `GeneratedImage`, `StoreFileCounts`, `Citation` variants

## 6) Execution Flow (Important for Porting)

## 6.1 Text agent prompt flow

1. Agent calls `Promptable::prompt()`.
2. Provider/model set resolved via attributes/methods + failover list.
3. Provider `GeneratesText::prompt()` builds pipeline middleware.
4. Prompt + prior messages assembled; tool invocation callbacks attached.
5. Gateway (`PrismGateway`) executes provider call.
6. Vendor response normalized to SDK response + events dispatched.

## 6.2 Streaming flow

1. `Promptable::stream()` -> provider `StreamsText::stream()`.
2. Stream events from Prism converted to SDK stream event objects.
3. `StreamableAgentResponse` can be iterated, SSE-streamed, or converted to Vercel protocol.
4. Final stream aggregate produces `StreamedAgentResponse`.

## 6.3 Queue flow

- `Promptable::queue()` and modality `Pending*::queue()` dispatch queue jobs.
- `Jobs/Concerns/InvokesQueuedResponseCallbacks` carries `then/catch` callbacks.
- `FakePendingDispatch` allows queue API in fake mode.

## 6.4 Failover flow

- Provider/model can be `Lab`, string, or list map.
- On `FailoverableException` (`RateLimitedException`, `ProviderOverloadedException`), next provider/model candidate is tried.
- Events: `ProviderFailedOver`, `AgentFailedOver`.

## 6.5 Testing/fake flow

- `AiManager` has per-surface fake gateways and prompt/operation recording concerns.
- Assertions available across all surfaces (`assertGenerated`, `assertQueued`, `assertStored`, etc.).
- Fake gateways optionally reject stray operations (`preventStray...`).

## 7) Test Surface and Coverage

- Strong feature tests for public SDK behavior across modalities and fakes.
- Unit tests focus on Prism conversion helpers (tool arg normalization, stream event mapping, usage mapping).
- Integration tests use real providers for many operations (requires external credentials).
- Command generation tests cover stub scaffolding.

Coverage emphasis is behavioral API consistency, not deep static correctness.

## 8) Findings and Risks (for Intake Decisions)

## 8.1 High-risk defects observed in current source

1. Extra argument passed to `addProviderTools`
- Call sites: `src/Gateway/Prism/PrismGateway.php` (two call sites pass 4 args)
- Definition: `src/Gateway/Prism/Concerns/AddsToolsToPrismRequests.php` (accepts 3 args)
- Impact: likely runtime argument-count error in tool-using prompt paths.

2. Undefined `isImage` call in `PrismGateway`
- File: `src/Gateway/Prism/PrismGateway.php`
- `toPrismImageAttachments()` calls `static::isImage($attachment)` but method is not defined in class/trait.
- Impact: runtime fatal path for uploaded image attachments.

3. Missing `Base64Document` import in `Store`
- File: `src/Store.php`
- Uses `Base64Document::fromUpload(...)` without importing `Laravel\Ai\Files\Base64Document`.
- Impact: runtime class resolution failure when adding uploaded files to stores.

4. Unit test helper mismatch in `AddsToolsToPrismRequestsTest`
- File: `tests/Unit/Gateway/Prism/AddsToolsToPrismRequestsTest.php`
- Anonymous helper defines `test_invoke_tool(...)` but test calls `testInvokeTool(...)`.
- Impact: test itself appears broken.

## 8.2 Design constraints to preserve if harvesting concepts

- Capability-first interfaces are clean; keep them decoupled from framework runtime.
- Event normalization is useful; transport-specific output protocol adapters should be separate layers.
- Fake gateways are valuable but currently tightly coupled to PHPUnit assertion style and Laravel facades.

## 9) What to Bring into OpenAgents (Recommended Harvest)

## 9.1 Strong candidates

1. **Capability contract pattern**
- Separate traits/interfaces per modality (`Text`, `Image`, `Audio`, `Embeddings`, etc.).
- Good fit for Rust trait-based client/server abstractions.

2. **Provider/model failover semantics**
- Ordered provider/model lists with explicit failoverable exceptions.
- Map to OpenAgents runtime policies as deterministic fallback plans.

3. **Normalized stream event schema**
- Start/delta/end/tool/reasoning events are a solid cross-provider abstraction.
- Keep mapping at provider adapter boundary; emit proto-governed events in OpenAgents.

4. **Tool invocation lifecycle instrumentation**
- Distinct “invoking” and “invoked” events with invocation IDs are operationally useful.

5. **Fake gateway testing strategy**
- Per-capability fake adapters + operation recording + assertion helpers can become Rust test harness adapters.

6. **Conversation store contract boundary**
- Store interface allows swapping persistence backend cleanly.

## 9.2 Medium candidates (adapt heavily)

1. **Provider-tool abstraction** (`WebSearch`, `WebFetch`, `FileSearch`)
- Keep concept, but move to proto-first typed contracts.

2. **Queued response callback ergonomics**
- Nice app API; adapt to Rust async job abstractions rather than PHP closures.

3. **File/store orchestration API**
- Useful UX, but provider-specific ID semantics need explicit typed models.

## 9.3 Avoid direct reuse

1. Laravel macro/facade/container/event/broadcast/database/storage coupling
2. Prism-specific object model as canonical domain model
3. Stringly typed provider/tool IDs without typed schema contracts

## 10) Porting Notes Against OpenAgents Constraints

For OpenAgents Rust-era constraints, harvest should be **pattern-level**, then reimplement with:

- Proto-first contracts
- Rust-owned domain/wire models
- Explicit runtime/control-plane boundaries
- WS/HTTP semantics conforming to existing invariants

Do not ingest this package’s framework glue as-is.

## 11) Full Directory Inventory (Non-`.git`)

```text
.gitattributes
.gitignore
art/logo.svg
composer.json
config/ai.php
database/migrations/2026_01_11_000001_create_agent_conversations_table.php
functions.php
LICENSE.md
phpunit.xml.dist
README.md
resources/boost/guidelines/core.blade.php
resources/boost/skills/ai-sdk-development/SKILL.md
src/Ai.php
src/AiManager.php
src/AiServiceProvider.php
src/AnonymousAgent.php
src/Attributes/MaxSteps.php
src/Attributes/MaxTokens.php
src/Attributes/Model.php
src/Attributes/Provider.php
src/Attributes/Temperature.php
src/Attributes/Timeout.php
src/Attributes/UseCheapestModel.php
src/Attributes/UseSmartestModel.php
src/Audio.php
src/Concerns/InteractsWithFakeAgents.php
src/Concerns/InteractsWithFakeAudio.php
src/Concerns/InteractsWithFakeEmbeddings.php
src/Concerns/InteractsWithFakeFiles.php
src/Concerns/InteractsWithFakeImages.php
src/Concerns/InteractsWithFakeReranking.php
src/Concerns/InteractsWithFakeStores.php
src/Concerns/InteractsWithFakeTranscriptions.php
src/Concerns/RemembersConversations.php
src/Concerns/Storable.php
src/Console/Commands/ChatCommand.php
src/Console/Commands/MakeAgentCommand.php
src/Console/Commands/MakeAgentMiddlewareCommand.php
src/Console/Commands/MakeToolCommand.php
src/Contracts/Agent.php
src/Contracts/Conversational.php
src/Contracts/ConversationStore.php
src/Contracts/Files/HasContent.php
src/Contracts/Files/HasMimeType.php
src/Contracts/Files/HasName.php
src/Contracts/Files/HasProviderId.php
src/Contracts/Files/StorableFile.php
src/Contracts/Files/TranscribableAudio.php
src/Contracts/Gateway/AudioGateway.php
src/Contracts/Gateway/EmbeddingGateway.php
src/Contracts/Gateway/FileGateway.php
src/Contracts/Gateway/Gateway.php
src/Contracts/Gateway/ImageGateway.php
src/Contracts/Gateway/RerankingGateway.php
src/Contracts/Gateway/StoreGateway.php
src/Contracts/Gateway/TextGateway.php
src/Contracts/Gateway/TranscriptionGateway.php
src/Contracts/HasMiddleware.php
src/Contracts/HasStructuredOutput.php
src/Contracts/HasTools.php
src/Contracts/Providers/AudioProvider.php
src/Contracts/Providers/EmbeddingProvider.php
src/Contracts/Providers/FileProvider.php
src/Contracts/Providers/ImageProvider.php
src/Contracts/Providers/RerankingProvider.php
src/Contracts/Providers/StoreProvider.php
src/Contracts/Providers/SupportsFileSearch.php
src/Contracts/Providers/SupportsWebFetch.php
src/Contracts/Providers/SupportsWebSearch.php
src/Contracts/Providers/TextProvider.php
src/Contracts/Providers/TranscriptionProvider.php
src/Contracts/Schemable.php
src/Contracts/Tool.php
src/Embeddings.php
src/Enums/Lab.php
src/Events/AddingFileToStore.php
src/Events/AgentFailedOver.php
src/Events/AgentPrompted.php
src/Events/AgentStreamed.php
src/Events/AudioGenerated.php
src/Events/CreatingStore.php
src/Events/EmbeddingsGenerated.php
src/Events/FileAddedToStore.php
src/Events/FileDeleted.php
src/Events/FileRemovedFromStore.php
src/Events/FileStored.php
src/Events/GeneratingAudio.php
src/Events/GeneratingEmbeddings.php
src/Events/GeneratingImage.php
src/Events/GeneratingTranscription.php
src/Events/ImageGenerated.php
src/Events/InvokingTool.php
src/Events/PromptingAgent.php
src/Events/ProviderFailedOver.php
src/Events/RemovingFileFromStore.php
src/Events/Reranked.php
src/Events/Reranking.php
src/Events/StoreCreated.php
src/Events/StoreDeleted.php
src/Events/StoringFile.php
src/Events/StreamingAgent.php
src/Events/ToolInvoked.php
src/Events/TranscriptionGenerated.php
src/Exceptions/AiException.php
src/Exceptions/FailoverableException.php
src/Exceptions/ProviderOverloadedException.php
src/Exceptions/RateLimitedException.php
src/FakePendingDispatch.php
src/Files.php
src/Files/Audio.php
src/Files/Base64Audio.php
src/Files/Base64Document.php
src/Files/Base64Image.php
src/Files/Concerns/CanBeRetrievedOrDeletedFromProvider.php
src/Files/Concerns/CanBeUploadedToProvider.php
src/Files/Concerns/HasRemoteContent.php
src/Files/Document.php
src/Files/File.php
src/Files/Image.php
src/Files/LocalAudio.php
src/Files/LocalDocument.php
src/Files/LocalImage.php
src/Files/ProviderDocument.php
src/Files/ProviderImage.php
src/Files/RemoteAudio.php
src/Files/RemoteDocument.php
src/Files/RemoteImage.php
src/Files/StoredAudio.php
src/Files/StoredDocument.php
src/Files/StoredImage.php
src/Gateway/AnthropicFileGateway.php
src/Gateway/CohereGateway.php
src/Gateway/Concerns/HandlesRateLimiting.php
src/Gateway/Concerns/PreparesStorableFiles.php
src/Gateway/ElevenLabsGateway.php
src/Gateway/FakeAudioGateway.php
src/Gateway/FakeEmbeddingGateway.php
src/Gateway/FakeFileGateway.php
src/Gateway/FakeImageGateway.php
src/Gateway/FakeRerankingGateway.php
src/Gateway/FakeStoreGateway.php
src/Gateway/FakeTextGateway.php
src/Gateway/FakeTranscriptionGateway.php
src/Gateway/GeminiFileGateway.php
src/Gateway/GeminiStoreGateway.php
src/Gateway/JinaGateway.php
src/Gateway/OpenAiFileGateway.php
src/Gateway/OpenAiStoreGateway.php
src/Gateway/Prism/Concerns/AddsToolsToPrismRequests.php
src/Gateway/Prism/Concerns/CreatesPrismTextRequests.php
src/Gateway/Prism/PrismCitations.php
src/Gateway/Prism/PrismException.php
src/Gateway/Prism/PrismGateway.php
src/Gateway/Prism/PrismMessages.php
src/Gateway/Prism/PrismSteps.php
src/Gateway/Prism/PrismStreamEvent.php
src/Gateway/Prism/PrismTool.php
src/Gateway/Prism/PrismUsage.php
src/Gateway/TextGenerationOptions.php
src/Gateway/VoyageAiGateway.php
src/Gateway/XaiImageGateway.php
src/Image.php
src/Jobs/BroadcastAgent.php
src/Jobs/Concerns/InvokesQueuedResponseCallbacks.php
src/Jobs/GenerateAudio.php
src/Jobs/GenerateEmbeddings.php
src/Jobs/GenerateImage.php
src/Jobs/GenerateTranscription.php
src/Jobs/InvokeAgent.php
src/Messages/AssistantMessage.php
src/Messages/Message.php
src/Messages/MessageRole.php
src/Messages/ToolResultMessage.php
src/Messages/UserMessage.php
src/Middleware/RememberConversation.php
src/Migrations/AiMigration.php
src/ObjectSchema.php
src/PendingResponses/PendingAudioGeneration.php
src/PendingResponses/PendingEmbeddingsGeneration.php
src/PendingResponses/PendingImageGeneration.php
src/PendingResponses/PendingReranking.php
src/PendingResponses/PendingTranscriptionGeneration.php
src/Promptable.php
src/Prompts/AgentPrompt.php
src/Prompts/AudioPrompt.php
src/Prompts/EmbeddingsPrompt.php
src/Prompts/ImagePrompt.php
src/Prompts/Prompt.php
src/Prompts/QueuedAudioPrompt.php
src/Prompts/QueuedEmbeddingsPrompt.php
src/Prompts/QueuedImagePrompt.php
src/Prompts/QueuedTranscriptionPrompt.php
src/Prompts/RerankingPrompt.php
src/Prompts/TranscriptionPrompt.php
src/Providers/AnthropicProvider.php
src/Providers/AzureOpenAiProvider.php
src/Providers/CohereProvider.php
src/Providers/Concerns/GeneratesAudio.php
src/Providers/Concerns/GeneratesEmbeddings.php
src/Providers/Concerns/GeneratesImages.php
src/Providers/Concerns/GeneratesText.php
src/Providers/Concerns/GeneratesTranscriptions.php
src/Providers/Concerns/HasAudioGateway.php
src/Providers/Concerns/HasEmbeddingGateway.php
src/Providers/Concerns/HasFileGateway.php
src/Providers/Concerns/HasImageGateway.php
src/Providers/Concerns/HasRerankingGateway.php
src/Providers/Concerns/HasStoreGateway.php
src/Providers/Concerns/HasTextGateway.php
src/Providers/Concerns/HasTranscriptionGateway.php
src/Providers/Concerns/ManagesFiles.php
src/Providers/Concerns/ManagesStores.php
src/Providers/Concerns/Reranks.php
src/Providers/Concerns/StreamsText.php
src/Providers/DeepSeekProvider.php
src/Providers/ElevenLabsProvider.php
src/Providers/GeminiProvider.php
src/Providers/GroqProvider.php
src/Providers/JinaProvider.php
src/Providers/MistralProvider.php
src/Providers/OllamaProvider.php
src/Providers/OpenAiProvider.php
src/Providers/OpenRouterProvider.php
src/Providers/Provider.php
src/Providers/Tools/FileSearch.php
src/Providers/Tools/FileSearchQuery.php
src/Providers/Tools/ProviderTool.php
src/Providers/Tools/WebFetch.php
src/Providers/Tools/WebSearch.php
src/Providers/VoyageAiProvider.php
src/Providers/XaiProvider.php
src/QueuedAgentPrompt.php
src/Reranking.php
src/Responses/AddedDocumentResponse.php
src/Responses/AgentResponse.php
src/Responses/AudioResponse.php
src/Responses/Concerns/CanStreamUsingVercelProtocol.php
src/Responses/Concerns/HasQueuedResponseCallbacks.php
src/Responses/Data/Citation.php
src/Responses/Data/FinishReason.php
src/Responses/Data/GeneratedImage.php
src/Responses/Data/Meta.php
src/Responses/Data/RankedDocument.php
src/Responses/Data/Step.php
src/Responses/Data/StoreFileCounts.php
src/Responses/Data/StructuredStep.php
src/Responses/Data/ToolCall.php
src/Responses/Data/ToolResult.php
src/Responses/Data/TranscriptionSegment.php
src/Responses/Data/UrlCitation.php
src/Responses/Data/Usage.php
src/Responses/EmbeddingsResponse.php
src/Responses/FileResponse.php
src/Responses/ImageResponse.php
src/Responses/ProvidesStructuredResponse.php
src/Responses/QueuedAgentResponse.php
src/Responses/QueuedAudioResponse.php
src/Responses/QueuedEmbeddingsResponse.php
src/Responses/QueuedImageResponse.php
src/Responses/QueuedTranscriptionResponse.php
src/Responses/RerankingResponse.php
src/Responses/StoredFileResponse.php
src/Responses/StreamableAgentResponse.php
src/Responses/StreamedAgentResponse.php
src/Responses/StructuredAgentResponse.php
src/Responses/StructuredTextResponse.php
src/Responses/TextResponse.php
src/Responses/TranscriptionResponse.php
src/Schema.php
src/Storage/DatabaseConversationStore.php
src/Store.php
src/Stores.php
src/Streaming/Events/Citation.php
src/Streaming/Events/Error.php
src/Streaming/Events/ProviderToolEvent.php
src/Streaming/Events/ReasoningDelta.php
src/Streaming/Events/ReasoningEnd.php
src/Streaming/Events/ReasoningStart.php
src/Streaming/Events/StreamEnd.php
src/Streaming/Events/StreamEvent.php
src/Streaming/Events/StreamStart.php
src/Streaming/Events/TextDelta.php
src/Streaming/Events/TextEnd.php
src/Streaming/Events/TextStart.php
src/Streaming/Events/ToolCall.php
src/Streaming/Events/ToolResult.php
src/StructuredAnonymousAgent.php
src/Tools/Request.php
src/Tools/SimilaritySearch.php
src/Transcription.php
stubs/agent.stub
stubs/middleware.stub
stubs/structured-agent.stub
stubs/tool.stub
tests/Feature/AgentAttributeTest.php
tests/Feature/AgentFakeIntegrationTest.php
tests/Feature/AgentFakeTest.php
tests/Feature/AgentIntegrationTest.php
tests/Feature/AgentMiddlewareTest.php
tests/Feature/Agents/AssistantAgent.php
tests/Feature/Agents/AttributeAgent.php
tests/Feature/Agents/ConversationalAgent.php
tests/Feature/Agents/SecondaryAssistantAgent.php
tests/Feature/Agents/StructuredAgent.php
tests/Feature/Agents/ToolUsingAgent.php
tests/Feature/AiManagerTest.php
tests/Feature/AiProviderEnumIntegrationTest.php
tests/Feature/AudioFakeTest.php
tests/Feature/AudioIntegrationTest.php
tests/Feature/Console/MakeAgentCommandTest.php
tests/Feature/Console/MakeAgentMiddlewareCommandTest.php
tests/Feature/Console/MakeToolCommandTest.php
tests/Feature/EmbeddingsFakeTest.php
tests/Feature/EmbeddingsIntegrationTest.php
tests/Feature/FakeJsonSchemaDataTest.php
tests/Feature/FileFakeTest.php
tests/Feature/FileIntegrationTest.php
tests/Feature/files/audio.mp3
tests/Feature/files/document.txt
tests/Feature/files/report.txt
tests/Feature/ImageFakeTest.php
tests/Feature/ImageIntegrationTest.php
tests/Feature/RerankingFakeTest.php
tests/Feature/RerankingIntegrationTest.php
tests/Feature/SimilaritySearchTest.php
tests/Feature/StoreFakeTest.php
tests/Feature/StoreIntegrationTest.php
tests/Feature/StreamableAgentResponseStreamingTest.php
tests/Feature/tmp/.gitignore
tests/Feature/Tools/FixedNumberGenerator.php
tests/Feature/Tools/RandomNumberGenerator.php
tests/Feature/TranscriptionFakeTest.php
tests/TestCase.php
tests/Unit/Gateway/Prism/AddsToolsToPrismRequestsTest.php
tests/Unit/Gateway/Prism/PrismStreamEventTest.php
tests/Unit/Gateway/Prism/PrismToolTest.php
tests/Unit/Gateway/Prism/PrismUsageTest.php

```
