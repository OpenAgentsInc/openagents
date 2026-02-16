<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Http\Requests\Api\CreateChatRequest;
use App\Services\PostHogService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
use Laravel\Ai\Contracts\ConversationStore;

class ChatController extends Controller
{
    public function index(Request $request): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $limit = max(1, min(200, (int) $request->integer('limit', 50)));

        $threads = DB::table('threads')
            ->where('user_id', $user->id)
            ->orderByDesc('updated_at')
            ->limit($limit)
            ->get(['id', 'title', 'created_at', 'updated_at'])
            ->map(fn ($thread): array => [
                'id' => (string) $thread->id,
                'title' => (string) ($thread->title ?: 'New conversation'),
                'createdAt' => $thread->created_at,
                'updatedAt' => $thread->updated_at,
            ])
            ->all();

        return response()->json(['data' => $threads]);
    }

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
                'createdAt' => $now->toISOString(),
                'updatedAt' => $now->toISOString(),
            ],
        ], 201);
    }

    public function show(Request $request, string $conversationId): JsonResponse
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
                'title' => (string) $conversation->title,
                'created_at' => $now,
                'updated_at' => $now,
            ]);

            $thread = (object) [
                'id' => $conversationId,
                'title' => (string) $conversation->title,
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
                    'createdAt' => $thread->created_at,
                    'updatedAt' => $thread->updated_at,
                ],
                'messages' => $messages,
                'runs' => $runs,
            ],
        ]);
    }

    public function messages(Request $request, string $conversationId): JsonResponse
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

    public function runs(Request $request, string $conversationId): JsonResponse
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

    public function runEvents(Request $request, string $conversationId, string $runId): JsonResponse
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
            ->get(['id', 'type', 'payload', 'created_at'])
            ->map(fn ($event): array => [
                'id' => (int) $event->id,
                'type' => (string) $event->type,
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
     * @return array<int, array<string, mixed>>
     */
    private function messagesForConversation(int $userId, string $conversationId): array
    {
        $messages = DB::table('messages')
            ->where('thread_id', $conversationId)
            ->where('user_id', $userId)
            ->orderBy('created_at')
            ->get(['id', 'run_id', 'role', 'content', 'meta', 'created_at', 'updated_at'])
            ->map(fn ($message): array => [
                'id' => (string) $message->id,
                'runId' => $message->run_id ? (string) $message->run_id : null,
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
            ->get(['id', 'run_id', 'role', 'content', 'meta', 'created_at', 'updated_at'])
            ->map(fn ($message): array => [
                'id' => (string) $message->id,
                'runId' => $message->run_id ? (string) $message->run_id : null,
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
    private function runsForConversation(int $userId, string $conversationId, int $limit): array
    {
        return DB::table('runs')
            ->where('thread_id', $conversationId)
            ->where('user_id', $userId)
            ->orderByDesc('created_at')
            ->limit($limit)
            ->get(['id', 'status', 'model_provider', 'model', 'usage', 'meta', 'error', 'started_at', 'completed_at', 'created_at', 'updated_at'])
            ->map(fn ($run): array => [
                'id' => (string) $run->id,
                'status' => (string) $run->status,
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
