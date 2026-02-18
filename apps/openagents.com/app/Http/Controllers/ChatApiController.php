<?php

namespace App\Http\Controllers;

use App\AI\RunOrchestrator;
use App\Services\GuestChatSessionService;
use App\Services\PostHogService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Log;

class ChatApiController extends Controller
{
    public function stream(Request $request)
    {
        /** @var GuestChatSessionService $guestService */
        $guestService = resolve(GuestChatSessionService::class);
        $user = $request->user();
        $authenticatedSession = $user !== null;

        $conversationId = $request->route('conversationId');

        if (! is_string($conversationId) || trim($conversationId) === '') {
            $conversationId = $request->query('conversationId');
        }

        if (! is_string($conversationId) || trim($conversationId) === '') {
            return $this->unprocessable('conversationId is required (route param or query param)');
        }

        $conversationId = trim($conversationId);

        if (! $user) {
            if (! $guestService->isGuestConversationId($conversationId)) {
                return $this->unauthorized();
            }

            // Non-blocking guest UX: allow first stream call to establish the
            // session guest id if the guest-session preflight is still in flight.
            $sessionGuestId = $guestService->ensureGuestConversationId($request, $conversationId);
            if ($sessionGuestId !== $conversationId) {
                return $this->unauthorized();
            }

            $guestService->ensureGuestConversationAndThread($conversationId);
            $user = $guestService->guestUser();
        }

        $conversationExists = $this->ensureConversationAccessibleForUser(
            $conversationId,
            (int) $user->getAuthIdentifier(),
            $guestService,
        );

        if (! $conversationExists) {
            return $this->notFound('Conversation not found or inaccessible. Start a new chat and try again.');
        }

        $rawMessages = $request->input('messages');

        if (! is_array($rawMessages) || $rawMessages === []) {
            return $this->unprocessable('messages must be a non-empty array');
        }

        $messages = $this->normalizeMessages($rawMessages);

        if ($messages === []) {
            return $this->unprocessable('messages must include at least one valid message');
        }

        // The AI SDK sends the full message list; use the most recent user message as the next prompt.
        $prompt = null;
        foreach (array_reverse($messages) as $m) {
            if (($m['role'] ?? null) === 'user' && trim((string) ($m['content'] ?? '')) !== '') {
                $prompt = trim((string) $m['content']);
                break;
            }
        }

        if ($prompt === null || $prompt === '') {
            return $this->unprocessable('A non-empty user message is required');
        }

        // PostHog: Track chat message sent
        $posthog = resolve(PostHogService::class);
        $posthog->capture($user->email, 'chat message sent', [
            'conversation_id' => $conversationId,
            'message_length' => strlen($prompt),
        ]);

        Log::info('Chat stream: starting', ['conversation_id' => $conversationId, 'prompt_length' => strlen($prompt)]);

        $orchestrator = resolve(RunOrchestrator::class);
        $response = $orchestrator->streamAutopilotRun($user, $conversationId, $prompt, $authenticatedSession);

        Log::info('Chat stream: response created', ['conversation_id' => $conversationId]);

        return $response;
    }

    /**
     * Ensure the given conversation is accessible for the user. If this is an adopted
     * guest conversation after in-chat login, migrate ownership on-demand.
     */
    private function ensureConversationAccessibleForUser(
        string $conversationId,
        int $userId,
        GuestChatSessionService $guestService,
    ): bool {
        $exists = DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->where('user_id', $userId)
            ->exists();

        if ($exists) {
            return true;
        }

        if (! $guestService->isGuestConversationId($conversationId)) {
            return false;
        }

        $guestUserId = (int) $guestService->guestUser()->getAuthIdentifier();
        if ($guestUserId <= 0 || $guestUserId === $userId) {
            return false;
        }

        /** @var int|null $ownerIdRaw */
        $ownerIdRaw = DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->value('user_id');

        // If no conversation exists yet, self-heal by creating one for this user.
        if ($ownerIdRaw === null) {
            $now = now();

            DB::transaction(function () use ($conversationId, $userId, $now): void {
                DB::table('agent_conversations')->insertOrIgnore([
                    'id' => $conversationId,
                    'user_id' => $userId,
                    'title' => 'Chat',
                    'created_at' => $now,
                    'updated_at' => $now,
                ]);

                DB::table('threads')->insertOrIgnore([
                    'id' => $conversationId,
                    'user_id' => $userId,
                    'autopilot_id' => null,
                    'title' => 'Chat',
                    'created_at' => $now,
                    'updated_at' => $now,
                ]);
            });

            return DB::table('agent_conversations')
                ->where('id', $conversationId)
                ->where('user_id', $userId)
                ->exists();
        }

        $ownerId = (int) $ownerIdRaw;

        if ($ownerId === $userId) {
            return true;
        }

        // Do not steal non-guest conversations from other real users.
        if ($ownerId !== $guestUserId) {
            return false;
        }

        $now = now();

        DB::transaction(function () use ($conversationId, $guestUserId, $userId, $now): void {
            DB::table('agent_conversations')
                ->where('id', $conversationId)
                ->where('user_id', $guestUserId)
                ->update([
                    'user_id' => $userId,
                    'updated_at' => $now,
                ]);

            DB::table('threads')
                ->where('id', $conversationId)
                ->where('user_id', $guestUserId)
                ->update([
                    'user_id' => $userId,
                    'updated_at' => $now,
                ]);

            DB::table('messages')
                ->where('thread_id', $conversationId)
                ->where('user_id', $guestUserId)
                ->update([
                    'user_id' => $userId,
                    'updated_at' => $now,
                ]);

            DB::table('runs')
                ->where('thread_id', $conversationId)
                ->where('user_id', $guestUserId)
                ->update([
                    'user_id' => $userId,
                    'updated_at' => $now,
                ]);

            DB::table('run_events')
                ->where('thread_id', $conversationId)
                ->where('user_id', $guestUserId)
                ->update([
                    'user_id' => $userId,
                ]);

            if (DB::getSchemaBuilder()->hasTable('agent_conversation_messages')) {
                DB::table('agent_conversation_messages')
                    ->where('conversation_id', $conversationId)
                    ->where('user_id', $guestUserId)
                    ->update([
                        'user_id' => $userId,
                        'updated_at' => $now,
                    ]);
            }
        });

        return DB::table('agent_conversations')
            ->where('id', $conversationId)
            ->where('user_id', $userId)
            ->exists();
    }

    /**
     * @param  array<int, mixed>  $rawMessages
     * @return array<int, array{role: string, content: string}>
     */
    private function normalizeMessages(array $rawMessages): array
    {
        $normalized = [];

        foreach ($rawMessages as $rawMessage) {
            if (! is_array($rawMessage)) {
                continue;
            }

            $role = $rawMessage['role'] ?? null;
            if (! is_string($role) || trim($role) === '') {
                continue;
            }

            $content = '';

            if (isset($rawMessage['content']) && is_string($rawMessage['content'])) {
                $content = $rawMessage['content'];
            } elseif (isset($rawMessage['parts']) && is_array($rawMessage['parts'])) {
                $content = $this->contentFromParts($rawMessage['parts']);
            }

            $normalized[] = [
                'role' => trim($role),
                'content' => trim($content),
            ];
        }

        return $normalized;
    }

    /**
     * @param  array<int, mixed>  $parts
     */
    private function contentFromParts(array $parts): string
    {
        $chunks = [];

        foreach ($parts as $part) {
            if (! is_array($part)) {
                continue;
            }

            $text = $part['text'] ?? null;
            if (is_string($text) && $text !== '') {
                $chunks[] = $text;
            }
        }

        return implode('', $chunks);
    }

    private function unauthorized(string $message = 'Unauthenticated.'): JsonResponse
    {
        return response()->json([
            'message' => $message,
        ], 401);
    }

    private function notFound(string $message = 'Not found.'): JsonResponse
    {
        return response()->json([
            'message' => $message,
        ], 404);
    }

    private function unprocessable(string $message): JsonResponse
    {
        return response()->json([
            'message' => $message,
            'errors' => [
                'messages' => [$message],
            ],
        ], 422);
    }
}
