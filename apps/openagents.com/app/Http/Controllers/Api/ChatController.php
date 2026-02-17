<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\ChatApiController;
use App\Http\Controllers\Controller;
use App\Http\Requests\Api\CreateChatRequest;
use App\OpenApi\Parameters\LimitQueryParameter;
use App\OpenApi\RequestBodies\ChatStreamRequestBody;
use App\OpenApi\RequestBodies\CreateChatRequestBody;
use App\OpenApi\Responses\CreatedDataObjectResponse;
use App\OpenApi\Responses\DataArrayResponse;
use App\OpenApi\Responses\DataObjectResponse;
use App\OpenApi\Responses\NotFoundResponse;
use App\OpenApi\Responses\SseStreamResponse;
use App\OpenApi\Responses\UnauthorizedResponse;
use App\OpenApi\Responses\ValidationErrorResponse;
use App\Services\PostHogService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
use Laravel\Ai\Contracts\ConversationStore;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class ChatController extends Controller
{
    /**
     * List authenticated user's conversation threads.
     *
     * Returns recent thread ids/titles sorted by update time. Use the `limit` query
     * parameter to bound result size.
     */
    #[OpenApi\Operation(tags: ['Chat'])]
    #[OpenApi\Parameters(factory: LimitQueryParameter::class)]
    #[OpenApi\Response(factory: DataArrayResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    public function index(Request $request): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $limit = max(1, min(200, (int) $request->integer('limit', 50)));

        $threadList = app(\App\Support\ChatThreadList::class);

        $threads = $threadList->forUser((int) $user->id, $limit)
            ->map(fn ($thread): array => [
                'id' => (string) $thread->id,
                'title' => $threadList->normalizeTitle($thread->title),
                'autopilotId' => is_string($thread->autopilot_id ?? null) ? (string) $thread->autopilot_id : null,
                'createdAt' => $thread->created_at,
                'updatedAt' => $thread->updated_at,
            ])
            ->all();

        return response()->json(['data' => $threads]);
    }

    /**
     * Create a new conversation thread.
     *
     * Creates a conversation container and persists an initial thread title.
     */
    #[OpenApi\Operation(tags: ['Chat'])]
    #[OpenApi\RequestBody(factory: CreateChatRequestBody::class)]
    #[OpenApi\Response(factory: CreatedDataObjectResponse::class, statusCode: 201)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function store(CreateChatRequest $request, ConversationStore $conversationStore, PostHogService $posthog): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $title = trim((string) ($request->validated()['title'] ?? ''));
        if ($title === '') {
            $title = 'New conversation';
        }

        $conversationId = (string) $conversationStore->storeConversation($user->id, $title);

        $now = now();

        DB::table('threads')->insert([
            'id' => $conversationId,
            'user_id' => $user->id,
            'autopilot_id' => null,
            'title' => $title,
            'created_at' => $now,
            'updated_at' => $now,
        ]);

        $posthog->capture($user->email, 'chat started', [
            'conversation_id' => $conversationId,
            'source' => 'api',
        ]);

        return response()->json([
            'data' => [
                'id' => $conversationId,
                'title' => $title,
                'autopilotId' => null,
                'createdAt' => $now->toISOString(),
                'updatedAt' => $now->toISOString(),
            ],
        ], 201);
    }

    /**
     * Get one conversation with messages and runs.
     *
     * Returns the thread metadata plus current message/run snapshots for the
     * authenticated user.
     */
    #[OpenApi\Operation(tags: ['Chat'])]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    public function show(Request $request, ?string $conversationId): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $conversation = DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->where('user_id', $user->id)
            ->first();

        if (! $conversation) {
            abort(404);
        }

        $thread = DB::table('threads')
            ->where('id', $conversationId)
            ->where('user_id', $user->id)
            ->first();

        if (! $thread) {
            $now = now();
            DB::table('threads')->insert([
                'id' => $conversationId,
                'user_id' => $user->id,
                'autopilot_id' => null,
                'title' => (string) $conversation->title,
                'created_at' => $now,
                'updated_at' => $now,
            ]);

            $thread = (object) [
                'id' => $conversationId,
                'title' => (string) $conversation->title,
                'autopilot_id' => null,
                'created_at' => $now,
                'updated_at' => $now,
            ];
        }

        $messages = $this->messagesForConversation((int) $user->id, $conversationId);
        $runs = $this->runsForConversation((int) $user->id, $conversationId, 100);

        return response()->json([
            'data' => [
                'conversation' => [
                    'id' => (string) $thread->id,
                    'title' => (string) $thread->title,
                    'autopilotId' => is_string($thread->autopilot_id ?? null) ? (string) $thread->autopilot_id : null,
                    'createdAt' => $thread->created_at,
                    'updatedAt' => $thread->updated_at,
                ],
                'messages' => $messages,
                'runs' => $runs,
            ],
        ]);
    }

    /**
     * List messages for a conversation.
     *
     * Returns normalized message records in chronological order.
     */
    #[OpenApi\Operation(tags: ['Chat'])]
    #[OpenApi\Response(factory: DataArrayResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    public function messages(Request $request, ?string $conversationId): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $conversationExists = DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->where('user_id', $user->id)
            ->exists();

        if (! $conversationExists) {
            abort(404);
        }

        return response()->json([
            'data' => $this->messagesForConversation((int) $user->id, $conversationId),
        ]);
    }

    /**
     * List runs for a conversation.
     *
     * Returns run metadata sorted newest-first. Use `limit` to cap returned rows.
     */
    #[OpenApi\Operation(tags: ['Chat'])]
    #[OpenApi\Parameters(factory: LimitQueryParameter::class)]
    #[OpenApi\Response(factory: DataArrayResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    public function runs(Request $request, ?string $conversationId): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $conversationExists = DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->where('user_id', $user->id)
            ->exists();

        if (! $conversationExists) {
            abort(404);
        }

        $limit = max(1, min(500, (int) $request->integer('limit', 100)));

        return response()->json([
            'data' => $this->runsForConversation((int) $user->id, $conversationId, $limit),
        ]);
    }

    /**
     * List events for a specific run.
     *
     * Returns the selected run plus ordered event payloads. Use `limit` to bound
     * event rows.
     */
    #[OpenApi\Operation(tags: ['Chat'])]
    #[OpenApi\Parameters(factory: LimitQueryParameter::class)]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    public function runEvents(Request $request, ?string $conversationId, string $runId): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $run = DB::table('runs')
            ->where('id', $runId)
            ->where('thread_id', $conversationId)
            ->where('user_id', $user->id)
            ->first();

        if (! $run) {
            abort(404);
        }

        $limit = max(1, min(1000, (int) $request->integer('limit', 250)));

        $events = DB::table('run_events')
            ->where('run_id', $runId)
            ->where('thread_id', $conversationId)
            ->where('user_id', $user->id)
            ->orderBy('id')
            ->limit($limit)
            ->get(['id', 'type', 'autopilot_id', 'actor_type', 'actor_autopilot_id', 'payload', 'created_at'])
            ->map(fn ($event): array => [
                'id' => (int) $event->id,
                'type' => (string) $event->type,
                'autopilotId' => is_string($event->autopilot_id ?? null) ? (string) $event->autopilot_id : null,
                'actorType' => (string) $event->actor_type,
                'actorAutopilotId' => is_string($event->actor_autopilot_id ?? null) ? (string) $event->actor_autopilot_id : null,
                'payload' => $this->decodeJsonColumn($event->payload),
                'createdAt' => (string) $event->created_at,
            ])
            ->values()
            ->all();

        return response()->json([
            'data' => [
                'run' => [
                    'id' => (string) $run->id,
                    'status' => (string) $run->status,
                    'autopilotId' => is_string($run->autopilot_id ?? null) ? (string) $run->autopilot_id : null,
                    'autopilotConfigVersion' => is_numeric($run->autopilot_config_version) ? (int) $run->autopilot_config_version : null,
                    'modelProvider' => $run->model_provider,
                    'model' => $run->model,
                    'usage' => $this->decodeJsonColumn($run->usage),
                    'meta' => $this->decodeJsonColumn($run->meta),
                    'error' => $run->error,
                    'startedAt' => $run->started_at,
                    'completedAt' => $run->completed_at,
                    'createdAt' => $run->created_at,
                    'updatedAt' => $run->updated_at,
                ],
                'events' => $events,
            ],
        ]);
    }

    /**
     * Stream a chat run over server-sent events.
     *
     * Expects the full message history and executes against the latest non-empty
     * user message.
     *
     * @param  string|null  $conversationId  Conversation identifier.
     */
    #[OpenApi\Operation(tags: ['Chat'])]
    #[OpenApi\RequestBody(factory: ChatStreamRequestBody::class)]
    #[OpenApi\Response(factory: SseStreamResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function stream(Request $request, ChatApiController $chatApiController, ?string $conversationId = null)
    {
        if (is_string($conversationId) && trim($conversationId) !== '') {
            $request->route()->setParameter('conversationId', $conversationId);
        }

        return $chatApiController->stream($request);
    }

    /**
     * @return array<int, array<string, mixed>>
     */
    private function messagesForConversation(int $userId, ?string $conversationId): array
    {
        $messages = DB::table('messages')
            ->where('thread_id', $conversationId)
            ->where('user_id', $userId)
            ->orderBy('created_at')
            ->get(['id', 'run_id', 'autopilot_id', 'role', 'content', 'meta', 'created_at', 'updated_at'])
            ->map(fn ($message): array => [
                'id' => (string) $message->id,
                'runId' => $message->run_id ? (string) $message->run_id : null,
                'autopilotId' => is_string($message->autopilot_id ?? null) ? (string) $message->autopilot_id : null,
                'role' => (string) $message->role,
                'content' => (string) $message->content,
                'meta' => $this->decodeJsonColumn($message->meta),
                'createdAt' => (string) $message->created_at,
                'updatedAt' => (string) $message->updated_at,
            ])
            ->all();

        if ($messages !== []) {
            return $messages;
        }

        $legacy = DB::table('agent_conversation_messages')
            ->where('conversation_id', $conversationId)
            ->where('user_id', $userId)
            ->orderBy('created_at')
            ->get(['id', 'role', 'content', 'created_at', 'updated_at']);

        if ($legacy->isNotEmpty()) {
            foreach ($legacy as $legacyMessage) {
                DB::table('messages')->updateOrInsert([
                    'id' => $legacyMessage->id,
                ], [
                    'thread_id' => $conversationId,
                    'run_id' => null,
                    'user_id' => $userId,
                    'autopilot_id' => null,
                    'role' => $legacyMessage->role,
                    'content' => $legacyMessage->content,
                    'meta' => null,
                    'created_at' => $legacyMessage->created_at,
                    'updated_at' => $legacyMessage->updated_at,
                ]);
            }
        }

        return DB::table('messages')
            ->where('thread_id', $conversationId)
            ->where('user_id', $userId)
            ->orderBy('created_at')
            ->get(['id', 'run_id', 'autopilot_id', 'role', 'content', 'meta', 'created_at', 'updated_at'])
            ->map(fn ($message): array => [
                'id' => (string) $message->id,
                'runId' => $message->run_id ? (string) $message->run_id : null,
                'autopilotId' => is_string($message->autopilot_id ?? null) ? (string) $message->autopilot_id : null,
                'role' => (string) $message->role,
                'content' => (string) $message->content,
                'meta' => $this->decodeJsonColumn($message->meta),
                'createdAt' => (string) $message->created_at,
                'updatedAt' => (string) $message->updated_at,
            ])
            ->all();
    }

    /**
     * @return array<int, array<string, mixed>>
     */
    private function runsForConversation(int $userId, ?string $conversationId, int $limit): array
    {
        return DB::table('runs')
            ->where('thread_id', $conversationId)
            ->where('user_id', $userId)
            ->orderByDesc('created_at')
            ->limit($limit)
            ->get(['id', 'status', 'autopilot_id', 'autopilot_config_version', 'model_provider', 'model', 'usage', 'meta', 'error', 'started_at', 'completed_at', 'created_at', 'updated_at'])
            ->map(fn ($run): array => [
                'id' => (string) $run->id,
                'status' => (string) $run->status,
                'autopilotId' => is_string($run->autopilot_id ?? null) ? (string) $run->autopilot_id : null,
                'autopilotConfigVersion' => is_numeric($run->autopilot_config_version) ? (int) $run->autopilot_config_version : null,
                'modelProvider' => $run->model_provider,
                'model' => $run->model,
                'usage' => $this->decodeJsonColumn($run->usage),
                'meta' => $this->decodeJsonColumn($run->meta),
                'error' => $run->error,
                'startedAt' => $run->started_at,
                'completedAt' => $run->completed_at,
                'createdAt' => $run->created_at,
                'updatedAt' => $run->updated_at,
            ])
            ->all();
    }

    /**
     * @return array<string, mixed>|null
     */
    private function decodeJsonColumn(mixed $value): ?array
    {
        if (! is_string($value) || trim($value) === '') {
            return null;
        }

        $decoded = json_decode($value, true);

        return is_array($decoded) ? $decoded : null;
    }
}
