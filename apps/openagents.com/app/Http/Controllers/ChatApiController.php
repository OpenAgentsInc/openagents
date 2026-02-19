<?php

namespace App\Http\Controllers;

use App\AI\Runtime\RuntimeClient;
use App\Models\User;
use App\Services\GuestChatSessionService;
use App\Services\PostHogService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Str;
use Symfony\Component\HttpFoundation\StreamedResponse;

class ChatApiController extends Controller
{
    public function stream(Request $request)
    {
        /** @var GuestChatSessionService $guestService */
        $guestService = resolve(GuestChatSessionService::class);
        $user = $this->resolveAuthenticatedUser($request);
        $authenticatedSession = $user !== null;

        $conversationId = $request->route('conversationId');

        if (! is_string($conversationId) || trim($conversationId) === '') {
            $conversationId = $request->query('conversationId');
        }

        if (! is_string($conversationId) || trim($conversationId) === '') {
            return $this->unprocessable('conversationId is required (route param or query param)');
        }

        $conversationId = trim($conversationId);

        $this->logAuthDebug($request, 'stream.auth_resolution', [
            'conversation_id' => $conversationId,
            'resolved_user_id' => $user?->id,
            'authenticated_session' => $authenticatedSession,
            'guard_authenticated' => $request->user() instanceof User,
        ]);

        if (! $user) {
            if (! $guestService->isGuestConversationId($conversationId)) {
                return $this->unauthorized();
            }

            // Non-blocking guest UX: allow first stream call to establish the
            // session guest id if the guest-session preflight is still in flight.
            $sessionGuestId = $guestService->ensureGuestConversationId($request, $conversationId);

            // If the requested id was stale or unusable, continue with the valid
            // guest session id instead of failing the request.
            if ($sessionGuestId !== $conversationId) {
                $conversationId = $sessionGuestId;
            }

            $guestService->ensureGuestConversationAndThread($conversationId);

            // If ownership changed between preflight and stream (race), rotate to a
            // fresh guest conversation id and continue instead of surfacing a 404.
            if (! $guestService->guestOwnsConversation($conversationId)) {
                $conversationId = $guestService->rotateGuestConversationId($request);
                $guestService->ensureGuestConversationAndThread($conversationId);
            }

            $user = $guestService->guestUser();
            $this->logAuthDebug($request, 'stream.guest_fallback', [
                'conversation_id' => $conversationId,
                'resolved_user_id' => $user->id,
                'authenticated_session' => false,
            ]);
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

        if (! $authenticatedSession && $this->requiresExplicitEmailTurn($prompt)) {
            return $this->streamGuestLoginEmailPrompt();
        }

        // PostHog: Track chat message sent
        $posthog = resolve(PostHogService::class);
        $posthog->capture($user->email, 'chat message sent', [
            'conversation_id' => $conversationId,
            'message_length' => strlen($prompt),
        ]);

        Log::info('Chat stream: starting', ['conversation_id' => $conversationId, 'prompt_length' => strlen($prompt)]);

        $runtimeClient = resolve(RuntimeClient::class);
        $response = $runtimeClient->streamAutopilotRun($user, $conversationId, $prompt, $authenticatedSession);

        Log::info('Chat stream: response created', [
            'conversation_id' => $conversationId,
            'runtime_driver' => $runtimeClient->driverName(),
        ]);

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
        if ($guestUserId <= 0) {
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
     * Resolve the authenticated user from the web guard or chat session fallback.
     */
    private function resolveAuthenticatedUser(Request $request): ?User
    {
        $user = $request->user();
        if ($user instanceof User) {
            $this->logAuthDebug($request, 'resolve_user.guard_hit', [
                'resolved_user_id' => $user->id,
            ]);

            return $user;
        }

        if (! $request->hasSession()) {
            $this->logAuthDebug($request, 'resolve_user.no_session');

            return null;
        }

        $userId = (int) $request->session()->get('chat.auth_user_id', 0);
        if ($userId <= 0) {
            $this->logAuthDebug($request, 'resolve_user.no_chat_auth_user_id', [
                'chat_auth_user_id' => $userId,
            ]);

            return null;
        }

        $rehydrated = User::query()->find($userId);
        if (! $rehydrated instanceof User) {
            $request->session()->forget('chat.auth_user_id');
            if (method_exists($request->session(), 'save')) {
                $request->session()->save();
            }

            $this->logAuthDebug($request, 'resolve_user.chat_user_missing', [
                'chat_auth_user_id' => $userId,
            ]);

            return null;
        }

        Auth::guard('web')->login($rehydrated);
        $this->logAuthDebug($request, 'resolve_user.rehydrated', [
            'resolved_user_id' => $rehydrated->id,
            'chat_auth_user_id' => $userId,
        ]);

        return $rehydrated;
    }

    /**
     * @param  array<string, mixed>  $context
     */
    private function logAuthDebug(Request $request, string $event, array $context = []): void
    {
        if (! app()->environment('local')) {
            return;
        }

        $session = $request->hasSession() ? $request->session() : null;

        Log::info('chat_api.'.$event, [
            'session_id' => $session && method_exists($session, 'getId') ? $session->getId() : null,
            'guard_authenticated' => Auth::guard('web')->check(),
            'guard_user_id' => Auth::guard('web')->id(),
            'chat_auth_user_id' => $session ? (int) $session->get('chat.auth_user_id', 0) : null,
            ...$context,
        ]);
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

    private function requiresExplicitEmailTurn(string $prompt): bool
    {
        $text = strtolower(trim($prompt));

        if ($text === '') {
            return false;
        }

        if ($this->containsEmailAddress($text)) {
            return false;
        }

        return preg_match('/\b(create\s+an?\s+account|create\s+account|sign\s*up|signup|log\s*in|login|sign\s*in)\b/i', $text) === 1;
    }

    private function containsEmailAddress(string $text): bool
    {
        return preg_match('/[A-Z0-9._%+\-]+@[A-Z0-9.\-]+\.[A-Z]{2,}/i', $text) === 1;
    }

    private function streamGuestLoginEmailPrompt(): StreamedResponse
    {
        $shouldFlush = ! app()->runningUnitTests();
        $messageId = 'msg_'.Str::lower(Str::random(12));
        $text = "To create an account, tell me your email address and I'll send a 6-digit code.";

        return response()->stream(function () use ($shouldFlush, $messageId, $text): void {
            $write = function (array $payload) use ($shouldFlush): void {
                echo 'data: '.json_encode($payload)."\n\n";
                if ($shouldFlush) {
                    if (ob_get_level() > 0) {
                        ob_flush();
                    }
                    flush();
                }
            };

            $write(['type' => 'start']);
            $write(['type' => 'start-step']);
            $write(['type' => 'text-start', 'id' => $messageId]);
            $write(['type' => 'text-delta', 'id' => $messageId, 'delta' => $text]);
            $write(['type' => 'text-end', 'id' => $messageId]);
            $write(['type' => 'finish-step']);
            echo "data: [DONE]\n\n";
            if ($shouldFlush) {
                if (ob_get_level() > 0) {
                    ob_flush();
                }
                flush();
            }
        }, 200, [
            'Cache-Control' => 'no-cache, no-transform',
            'Content-Type' => 'text/event-stream',
            'x-vercel-ai-ui-message-stream' => 'v1',
            'x-oa-guest-onboarding' => 'email-required',
        ]);
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
