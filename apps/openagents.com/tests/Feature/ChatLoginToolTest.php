<?php

use App\AI\Tools\ChatLoginTool;
use App\Models\User;
use App\Services\GuestChatSessionService;
use Illuminate\Http\Request as HttpRequest;
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use Laravel\Ai\Tools\Request as ToolRequest;
use WorkOS\Exception\GenericException;

afterEach(function () {
    \Mockery::close();
});

function configureWorkosForChatLoginToolTests(): void
{
    config()->set('services.workos.client_id', 'client_test_123');
    config()->set('services.workos.secret', 'sk_test_123');
    config()->set('services.workos.redirect_url', 'https://next.openagents.com/authenticate');
}

function bindHttpRequestWithSession(): void
{
    /** @var \Illuminate\Session\Store $session */
    $session = app('session.store');
    if (! $session->isStarted()) {
        $session->start();
    }

    $request = HttpRequest::create('/chat/test', 'POST');
    $request->setLaravelSession($session);

    app()->instance('request', $request);
}

test('chat_login status reports guest and pending states', function () {
    bindHttpRequestWithSession();

    $tool = new ChatLoginTool;

    $guest = json_decode($tool->handle(new ToolRequest([
        'action' => 'status',
    ])), true);

    expect($guest['status'] ?? null)->toBe('guest');
    expect($guest['authenticated'] ?? null)->toBeFalse();

    session()->put('auth.magic_auth', [
        'email' => 'chris@openagents.com',
        'user_id' => 'user_abc123',
    ]);

    $pending = json_decode($tool->handle(new ToolRequest([
        'action' => 'status',
    ])), true);

    expect($pending['status'] ?? null)->toBe('pending_verification');
    expect($pending['pendingEmail'] ?? null)->toBe('chris@openagents.com');
});

test('chat_login send_code stores pending magic auth in session', function () {
    configureWorkosForChatLoginToolTests();
    bindHttpRequestWithSession();

    $workos = \Mockery::mock('overload:WorkOS\\UserManagement');
    $workos->shouldReceive('createMagicAuth')
        ->once()
        ->with('chris@openagents.com')
        ->andReturn((object) [
            'userId' => 'user_abc123',
        ]);

    $tool = new ChatLoginTool;

    $result = json_decode($tool->handle(new ToolRequest([
        'action' => 'send_code',
        'email' => 'chris@openagents.com',
    ])), true);

    expect($result['status'] ?? null)->toBe('pending_verification');
    expect($result['pendingEmail'] ?? null)->toBe('chris@openagents.com');

    expect(session('auth.magic_auth.email'))->toBe('chris@openagents.com');
    expect(session('auth.magic_auth.user_id'))->toBe('user_abc123');
});

test('chat_login send_code surfaces provider errors', function () {
    configureWorkosForChatLoginToolTests();
    bindHttpRequestWithSession();

    $workos = \Mockery::mock('overload:WorkOS\\UserManagement');
    $workos->shouldReceive('createMagicAuth')
        ->once()
        ->andThrow(new GenericException('provider down'));

    $tool = new ChatLoginTool;

    $result = json_decode($tool->handle(new ToolRequest([
        'action' => 'send_code',
        'email' => 'chris@openagents.com',
    ])), true);

    expect($result['status'] ?? null)->toBe('failed');
    expect($result['denyCode'] ?? null)->toBe('send_code_failed');
});

test('chat_login verify_code authenticates and adopts guest conversation ownership', function () {
    configureWorkosForChatLoginToolTests();
    bindHttpRequestWithSession();

    $guestService = resolve(GuestChatSessionService::class);
    $guestUser = $guestService->guestUser();

    $conversationId = 'g-'.substr(str_replace('-', '', (string) Str::uuid7()), 0, 32);
    session()->put('chat.guest.conversation_id', $conversationId);
    $guestService->ensureGuestConversationAndThread($conversationId);

    DB::table('messages')->insert([
        'id' => (string) Str::uuid7(),
        'thread_id' => $conversationId,
        'run_id' => null,
        'user_id' => $guestUser->id,
        'role' => 'user',
        'content' => 'hello from guest',
        'meta' => null,
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    session()->put('auth.magic_auth', [
        'email' => 'chris@openagents.com',
        'user_id' => 'user_abc123',
    ]);

    $workosUser = (object) [
        'id' => 'user_abc123',
        'email' => 'chris@openagents.com',
        'firstName' => 'Chris',
        'lastName' => 'David',
        'profilePictureUrl' => 'https://example.com/avatar.png',
    ];

    $authResponse = (object) [
        'user' => $workosUser,
        'accessToken' => 'access_token_123',
        'refreshToken' => 'refresh_token_123',
    ];

    $workos = \Mockery::mock('overload:WorkOS\\UserManagement');
    $workos->shouldReceive('authenticateWithMagicAuth')
        ->once()
        ->with('client_test_123', '123456', 'user_abc123', \Mockery::any(), \Mockery::any())
        ->andReturn($authResponse);

    $tool = new ChatLoginTool;

    $result = json_decode($tool->handle(new ToolRequest([
        'action' => 'verify_code',
        'code' => '123456',
    ])), true);

    expect($result['status'] ?? null)->toBe('authenticated');
    expect($result['authenticated'] ?? null)->toBeTrue();

    $user = User::query()->where('email', 'chris@openagents.com')->firstOrFail();

    $this->assertAuthenticatedAs($user);
    expect(session('workos_access_token'))->toBe('access_token_123');
    expect(session('workos_refresh_token'))->toBe('refresh_token_123');
    expect(session('chat.auth_user_id'))->toBe((int) $user->id);

    expect(DB::table('agent_conversations')
        ->where('id', $conversationId)
        ->where('user_id', $user->id)
        ->exists())->toBeTrue();

    expect(DB::table('threads')
        ->where('id', $conversationId)
        ->where('user_id', $user->id)
        ->exists())->toBeTrue();

    expect(DB::table('messages')
        ->where('thread_id', $conversationId)
        ->where('user_id', $user->id)
        ->exists())->toBeTrue();
});

test('chat_login send_code persists pending session when invoked inside streamed response', function () {
    configureWorkosForChatLoginToolTests();

    $workos = \Mockery::mock('overload:WorkOS\\UserManagement');
    $workos->shouldReceive('createMagicAuth')
        ->once()
        ->with('chris@openagents.com')
        ->andReturn((object) [
            'userId' => 'user_stream_abc123',
        ]);

    $streamPath = '/_test/chat-login/'.Str::lower(Str::random(12)).'/stream-send-code';
    $pendingPath = '/_test/chat-login/'.Str::lower(Str::random(12)).'/pending';

    \Illuminate\Support\Facades\Route::post($streamPath, function (HttpRequest $request) {
        return response()->stream(function () use ($request): void {
            $tool = new ChatLoginTool;
            echo $tool->handle(new ToolRequest([
                'action' => 'send_code',
                'email' => (string) $request->input('email'),
            ]));
        }, 200, [
            'Content-Type' => 'text/event-stream',
        ]);
    })->middleware('web');

    \Illuminate\Support\Facades\Route::get($pendingPath, function () {
        return response()->json([
            'pending' => session('auth.magic_auth'),
        ]);
    })->middleware('web');

    $stream = $this->post($streamPath, [
        'email' => 'chris@openagents.com',
    ]);

    $stream->assertOk();
    $stream->streamedContent();

    $this->get($pendingPath)
        ->assertOk()
        ->assertJsonPath('pending.email', 'chris@openagents.com')
        ->assertJsonPath('pending.user_id', 'user_stream_abc123');
});

test('chat_login status rehydrates authenticated user from chat session key', function () {
    bindHttpRequestWithSession();

    $user = User::factory()->create([
        'email' => 'rehydrate@openagents.com',
    ]);

    session()->put('chat.auth_user_id', (int) $user->id);

    Auth::guard('web')->logout();

    $tool = new ChatLoginTool;

    $result = json_decode($tool->handle(new ToolRequest([
        'action' => 'status',
    ])), true);

    expect($result['status'] ?? null)->toBe('authenticated');
    expect((bool) ($result['authenticated'] ?? false))->toBeTrue();
    expect((int) ($result['user']['id'] ?? 0))->toBe((int) $user->id);

    $this->assertAuthenticatedAs($user);
});
