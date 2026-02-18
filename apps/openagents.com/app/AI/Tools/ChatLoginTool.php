<?php

namespace App\AI\Tools;

use App\Models\User;
use App\Services\GuestChatSessionService;
use App\Services\MagicAuthService;
use App\Services\PostHogService;
use Illuminate\Contracts\JsonSchema\JsonSchema;
use Illuminate\Contracts\Session\Session;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\DB;
use Illuminate\Validation\ValidationException;
use Laravel\Ai\Contracts\Tool;
use Laravel\Ai\Tools\Request;
use Throwable;

class ChatLoginTool implements Tool
{
    private const TOOL_NAME = 'chat_login';

    public function name(): string
    {
        return self::TOOL_NAME;
    }

    public function description(): string
    {
        return 'Complete in-chat email login using WorkOS email codes. Use action=status, then send_code with email, then verify_code with the 6-digit code.';
    }

    public function handle(Request $request): string
    {
        $action = strtolower(trim((string) $request->string('action', 'status')));

        return $this->encode(match ($action) {
            'status' => $this->status(),
            'send_code' => $this->sendCode($request),
            'verify_code' => $this->verifyCode($request),
            default => [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => $action,
                'denyCode' => 'invalid_action',
                'message' => 'action must be one of: status, send_code, verify_code.',
            ],
        });
    }

    public function schema(JsonSchema $schema): array
    {
        return [
            'action' => $schema
                ->string()
                ->description('status: check auth state. send_code: send email code. verify_code: verify 6-digit code and authenticate.')
                ->enum(['status', 'send_code', 'verify_code'])
                ->default('status')
                ->required(),
            'email' => $schema
                ->string()
                ->description('Required for action=send_code. Email address to receive the login code.'),
            'code' => $schema
                ->string()
                ->description('Required for action=verify_code. 6-digit code from email.'),
        ];
    }

    /**
     * @return array<string, mixed>
     */
    private function status(): array
    {
        $session = $this->sessionOrNull();
        if (! $session) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'status',
                'denyCode' => 'session_unavailable',
                'message' => 'No active HTTP session is available for chat login.',
            ];
        }

        $user = $this->resolveAuthenticatedUser($session);
        $pending = $this->pendingMagicAuth($session);

        if ($user instanceof User) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'authenticated',
                'action' => 'status',
                'authenticated' => true,
                'pending' => false,
                'user' => [
                    'id' => $user->id,
                    'email' => $user->email,
                    'name' => $user->name,
                    'handle' => $user->handle,
                ],
            ];
        }

        return [
            'toolName' => self::TOOL_NAME,
            'status' => $pending !== null ? 'pending_verification' : 'guest',
            'action' => 'status',
            'authenticated' => false,
            'pending' => $pending !== null,
            'pendingEmail' => $pending['email'] ?? null,
        ];
    }

    /**
     * @return array<string, mixed>
     */
    private function sendCode(Request $request): array
    {
        $session = $this->sessionOrNull();
        if (! $session) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'send_code',
                'denyCode' => 'session_unavailable',
                'message' => 'No active HTTP session is available for chat login.',
            ];
        }

        if ($this->resolveAuthenticatedUser($session) instanceof User) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'already_authenticated',
                'action' => 'send_code',
                'authenticated' => true,
            ];
        }

        $email = strtolower(trim((string) $request->string('email', '')));
        if ($email === '' || ! filter_var($email, FILTER_VALIDATE_EMAIL)) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'send_code',
                'denyCode' => 'invalid_email',
                'message' => 'Provide a valid email address.',
            ];
        }

        try {
            $pending = resolve(MagicAuthService::class)->startMagicCode($email);
        } catch (ValidationException $exception) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'send_code',
                'denyCode' => 'send_code_failed',
                'message' => $this->firstValidationMessage($exception, 'email') ?? 'Unable to send code right now. Try again.',
            ];
        }

        $session->put('auth.magic_auth', $pending);
        $this->persistSession($session);

        $posthog = resolve(PostHogService::class);
        $posthog->capture($email, 'login code sent', [
            'method' => 'magic_auth',
            'source' => 'chat_login_tool',
        ]);

        return [
            'toolName' => self::TOOL_NAME,
            'status' => 'pending_verification',
            'action' => 'send_code',
            'authenticated' => false,
            'pending' => true,
            'pendingEmail' => $email,
            'message' => "Code sent to {$email}. Ask the user for the 6-digit code.",
        ];
    }

    /**
     * @return array<string, mixed>
     */
    private function verifyCode(Request $request): array
    {
        $session = $this->sessionOrNull();
        if (! $session) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'verify_code',
                'denyCode' => 'session_unavailable',
                'message' => 'No active HTTP session is available for chat login.',
            ];
        }

        if ($this->resolveAuthenticatedUser($session) instanceof User) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'already_authenticated',
                'action' => 'verify_code',
                'authenticated' => true,
            ];
        }

        $pending = $this->pendingMagicAuth($session);
        if ($pending === null) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'verify_code',
                'denyCode' => 'pending_session_missing',
                'message' => 'No pending sign-in session. Send a code first.',
            ];
        }

        $code = preg_replace('/\s+/', '', trim((string) $request->string('code', '')));
        if (! is_string($code) || preg_match('/^\d{6}$/', $code) !== 1) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'verify_code',
                'denyCode' => 'invalid_code',
                'message' => 'Provide the 6-digit verification code from email.',
            ];
        }

        $httpRequest = request();
        $ipAddress = is_object($httpRequest) && method_exists($httpRequest, 'ip')
            ? (string) ($httpRequest->ip() ?? '')
            : '';
        $userAgent = is_object($httpRequest) && method_exists($httpRequest, 'userAgent')
            ? (string) ($httpRequest->userAgent() ?? '')
            : '';

        try {
            $verified = resolve(MagicAuthService::class)->verifyMagicCode(
                code: $code,
                pendingUserId: $pending['user_id'],
                ipAddress: $ipAddress,
                userAgent: $userAgent,
                pendingEmail: $pending['email'] ?? null,
            );
        } catch (ValidationException $exception) {
            return [
                'toolName' => self::TOOL_NAME,
                'status' => 'failed',
                'action' => 'verify_code',
                'denyCode' => 'verify_failed',
                'message' => $this->firstValidationMessage($exception, 'code') ?? 'Verification failed. Request a new code and try again.',
            ];
        }

        /** @var User $user */
        $user = $verified['user'];
        $isNewUser = (bool) ($verified['is_new_user'] ?? false);
        $accessToken = (string) ($verified['access_token'] ?? '');
        $refreshToken = (string) ($verified['refresh_token'] ?? '');

        $this->adoptGuestConversationIfNeeded($session, $user);

        Auth::guard('web')->login($user);
        $session->put('workos_access_token', $accessToken);
        $session->put('workos_refresh_token', $refreshToken);
        $session->put('chat.auth_user_id', (int) $user->getAuthIdentifier());
        $session->forget('auth.magic_auth');
        $session->forget('chat.guest.conversation_id');
        // Do not regenerate session token here: we are inside a streamed response, so the
        // client cannot receive a new Set-Cookie. Keeping the same session ID lets the
        // existing cookie continue to identify the (now authenticated) session on refresh.
        $this->persistSession($session);

        $posthog = resolve(PostHogService::class);
        $posthog->identify($user->email, $user->getPostHogProperties());

        if ($isNewUser) {
            $posthog->capture($user->email, 'user signed up', [
                'signup_method' => 'magic_auth',
                'source' => 'chat_login_tool',
            ]);
        } else {
            $posthog->capture($user->email, 'user logged in', [
                'login_method' => 'magic_auth',
                'source' => 'chat_login_tool',
            ]);
        }

        return [
            'toolName' => self::TOOL_NAME,
            'status' => 'authenticated',
            'action' => 'verify_code',
            'authenticated' => true,
            'user' => [
                'id' => $user->id,
                'email' => $user->email,
                'name' => $user->name,
                'handle' => $user->handle,
            ],
            'message' => 'Authentication successful. Protected tools are now available on your next message.',
        ];
    }

    private function adoptGuestConversationIfNeeded(Session $session, User $user): void
    {
        $guestService = resolve(GuestChatSessionService::class);
        /** @var mixed $conversationIdRaw */
        $conversationIdRaw = $session->get('chat.guest.conversation_id');

        if (! $guestService->isGuestConversationId($conversationIdRaw)) {
            return;
        }

        $conversationId = strtolower(trim((string) $conversationIdRaw));
        $guestUser = $guestService->guestUser();
        $guestUserId = (int) $guestUser->getAuthIdentifier();
        $targetUserId = (int) $user->getAuthIdentifier();

        if ($guestUserId <= 0 || $targetUserId <= 0 || $guestUserId === $targetUserId) {
            return;
        }

        $now = now();

        DB::transaction(function () use ($conversationId, $guestUserId, $targetUserId, $now): void {
            DB::table('agent_conversations')
                ->where('id', $conversationId)
                ->where('user_id', $guestUserId)
                ->update([
                    'user_id' => $targetUserId,
                    'updated_at' => $now,
                ]);

            DB::table('threads')
                ->where('id', $conversationId)
                ->where('user_id', $guestUserId)
                ->update([
                    'user_id' => $targetUserId,
                    'updated_at' => $now,
                ]);

            DB::table('messages')
                ->where('thread_id', $conversationId)
                ->where('user_id', $guestUserId)
                ->update([
                    'user_id' => $targetUserId,
                    'updated_at' => $now,
                ]);

            DB::table('runs')
                ->where('thread_id', $conversationId)
                ->where('user_id', $guestUserId)
                ->update([
                    'user_id' => $targetUserId,
                    'updated_at' => $now,
                ]);

            DB::table('run_events')
                ->where('thread_id', $conversationId)
                ->where('user_id', $guestUserId)
                ->update([
                    'user_id' => $targetUserId,
                ]);

            if (DB::getSchemaBuilder()->hasTable('agent_conversation_messages')) {
                DB::table('agent_conversation_messages')
                    ->where('conversation_id', $conversationId)
                    ->where('user_id', $guestUserId)
                    ->update([
                        'user_id' => $targetUserId,
                        'updated_at' => $now,
                    ]);
            }
        });
    }

    /**
     * Resolve authenticated user from guard or chat-session fallback key.
     */
    private function resolveAuthenticatedUser(Session $session): ?User
    {
        $user = Auth::guard('web')->user();
        if ($user instanceof User) {
            return $user;
        }

        $userId = (int) $session->get('chat.auth_user_id', 0);
        if ($userId <= 0) {
            return null;
        }

        $rehydrated = User::query()->find($userId);
        if (! $rehydrated instanceof User) {
            $session->forget('chat.auth_user_id');
            $this->persistSession($session);

            return null;
        }

        Auth::guard('web')->login($rehydrated);

        return $rehydrated;
    }

    private function pendingMagicAuth(Session $session): ?array
    {
        /** @var mixed $pending */
        $pending = $session->get('auth.magic_auth');

        if (! is_array($pending)) {
            return null;
        }

        $email = isset($pending['email']) && is_string($pending['email'])
            ? trim((string) $pending['email'])
            : null;
        $userId = isset($pending['user_id']) && is_string($pending['user_id'])
            ? trim((string) $pending['user_id'])
            : null;

        if (! is_string($email) || $email === '' || ! is_string($userId) || $userId === '') {
            return null;
        }

        return [
            'email' => strtolower($email),
            'user_id' => $userId,
        ];
    }

    private function firstValidationMessage(ValidationException $exception, string $field): ?string
    {
        $messages = $exception->errors()[$field] ?? null;
        if (! is_array($messages) || $messages === []) {
            return null;
        }

        $first = $messages[0] ?? null;

        return is_string($first) ? $first : null;
    }

    private function sessionOrNull(): ?Session
    {
        if (! app()->bound('session.store')) {
            return null;
        }

        /** @var mixed $session */
        $session = app('session.store');
        if (! $session instanceof Session) {
            return null;
        }

        if (method_exists($session, 'isStarted') && ! $session->isStarted()) {
            $session->start();
        }

        return $session;
    }

    private function persistSession(Session $session): void
    {
        try {
            if (method_exists($session, 'save')) {
                $session->save();
            }
        } catch (Throwable) {
            // Best-effort persistence for streamed tool calls.
        }
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    private function encode(array $payload): string
    {
        return json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE) ?: '{"toolName":"chat_login","status":"failed","denyCode":"encoding_failed"}';
    }
}
