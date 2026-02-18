<?php

use App\AI\Agents\AutopilotAgent;
use App\Models\User;
use App\Services\GuestChatSessionService;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use Laravel\Ai\Ai;

test('chat API streams Vercel protocol and persists final messages', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['Hello from fake agent']);

    $user = User::factory()->create();

    $conversationId = (string) Str::uuid7();

    DB::table('agent_conversations')->insert([
        'id' => $conversationId,
        'user_id' => $user->id,
        'title' => 'Test conversation',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    $response = $this->actingAs($user)->postJson('/api/chat?conversationId='.$conversationId, [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'Hello'],
        ],
    ]);

    $response->assertOk();

    $content = $response->streamedContent();

    expect($content)->toContain('data: {"type":"start"');
    expect($content)->toContain('data: {"type":"text-delta"');
    expect($content)->toContain('data: {"type":"finish"');
    expect($content)->toContain("data: [DONE]\n\n");

    // Conversation persistence from laravel/ai still works (Phase 1).
    $rows = DB::table('agent_conversation_messages')
        ->where('conversation_id', $conversationId)
        ->orderBy('created_at')
        ->get(['role', 'content']);

    expect($rows)->toHaveCount(2);
    expect($rows[0]->role)->toBe('user');
    expect($rows[1]->role)->toBe('assistant');
    expect($rows[1]->content)->toContain('Hello from fake agent');

    // Phase 2: canonical threads/runs/messages/run_events.
    expect(DB::table('threads')->where('id', $conversationId)->where('user_id', $user->id)->count())->toBe(1);

    $run = DB::table('runs')->where('thread_id', $conversationId)->where('user_id', $user->id)->first();
    expect($run)->not->toBeNull();
    expect($run->status)->toBe('completed');
    expect($run->autopilot_id)->toBeNull();
    expect($run->autopilot_config_version)->toBeNull();

    $msgs = DB::table('messages')
        ->where('thread_id', $conversationId)
        ->where('user_id', $user->id)
        ->orderBy('created_at')
        ->get(['role', 'content', 'autopilot_id']);

    expect($msgs)->toHaveCount(2);
    expect($msgs[0]->role)->toBe('user');
    expect($msgs[1]->role)->toBe('assistant');
    expect($msgs[1]->content)->toContain('Hello from fake agent');
    expect($msgs->pluck('autopilot_id')->unique()->values()->all())->toBe([null]);

    $events = DB::table('run_events')
        ->where('run_id', $run->id)
        ->where('user_id', $user->id)
        ->orderBy('id')
        ->get(['type', 'autopilot_id', 'actor_type', 'actor_autopilot_id']);

    expect($events->pluck('type')->all())->toContain('run_started');
    expect($events->pluck('type')->all())->toContain('model_stream_started');
    expect($events->pluck('type')->all())->toContain('model_finished');
    expect($events->pluck('type')->all())->toContain('run_completed');
    expect($events->pluck('autopilot_id')->unique()->values()->all())->toBe([null]);

    $runStarted = $events->firstWhere('type', 'run_started');
    expect($runStarted)->not->toBeNull();
    expect($runStarted->actor_type)->toBe('user');
    expect($runStarted->actor_autopilot_id)->toBeNull();

    $modelStarted = $events->firstWhere('type', 'model_stream_started');
    expect($modelStarted)->not->toBeNull();
    expect($modelStarted->actor_type)->toBe('system');
    expect($modelStarted->actor_autopilot_id)->toBeNull();
});

test('chat API accepts AI SDK parts payload and streams a response', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['Hello from fake agent via parts']);

    $user = User::factory()->create();

    $conversationId = (string) Str::uuid7();

    DB::table('agent_conversations')->insert([
        'id' => $conversationId,
        'user_id' => $user->id,
        'title' => 'Parts payload conversation',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    $response = $this->actingAs($user)->postJson('/api/chat?conversationId='.$conversationId, [
        'messages' => [
            [
                'id' => 'm1',
                'role' => 'user',
                'parts' => [
                    ['type' => 'text', 'text' => 'Hello from parts'],
                ],
            ],
        ],
    ]);

    $response->assertOk();

    $content = $response->streamedContent();

    expect($content)->toContain('data: {"type":"start"');
    expect($content)->toContain('data: {"type":"text-delta"');
    expect($content)->toContain('data: {"type":"finish"');
    expect($content)->toContain("data: [DONE]\n\n");

    $rows = DB::table('agent_conversation_messages')
        ->where('conversation_id', $conversationId)
        ->orderBy('created_at')
        ->get(['role', 'content']);

    expect($rows)->toHaveCount(2);
    expect($rows[0]->role)->toBe('user');
    expect($rows[0]->content)->toBe('Hello from parts');
    expect($rows[1]->role)->toBe('assistant');
    expect($rows[1]->content)->toContain('Hello from fake agent via parts');
});

test('chat API returns a JSON 422 error when user text is missing', function () {
    $user = User::factory()->create();

    $conversationId = (string) Str::uuid7();

    DB::table('agent_conversations')->insert([
        'id' => $conversationId,
        'user_id' => $user->id,
        'title' => 'Invalid payload conversation',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    $response = $this->actingAs($user)->postJson('/api/chat?conversationId='.$conversationId, [
        'messages' => [
            [
                'id' => 'm1',
                'role' => 'user',
                'parts' => [
                    ['type' => 'tool-call', 'name' => 'noop'],
                ],
            ],
        ],
    ]);

    $response->assertStatus(422)
        ->assertJsonPath('message', 'A non-empty user message is required');
});

test('chat API emits a visible fallback message when the model returns no text', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['']);

    $user = User::factory()->create();

    $conversationId = (string) Str::uuid7();

    DB::table('agent_conversations')->insert([
        'id' => $conversationId,
        'user_id' => $user->id,
        'title' => 'Empty model response conversation',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    $response = $this->actingAs($user)->postJson('/api/chat?conversationId='.$conversationId, [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'Hello'],
        ],
    ]);

    $response->assertOk();
    $content = $response->streamedContent();

    expect($content)->toContain("I couldn't generate a response from the model. Please try again.");
    expect($content)->toContain("data: [DONE]\n\n");

    $assistant = DB::table('messages')
        ->where('thread_id', $conversationId)
        ->where('user_id', $user->id)
        ->where('role', 'assistant')
        ->latest('created_at')
        ->first(['content']);

    expect($assistant)->not->toBeNull();
    expect((string) $assistant->content)->toContain("I couldn't generate a response from the model.");

    $run = DB::table('runs')
        ->where('thread_id', $conversationId)
        ->where('user_id', $user->id)
        ->latest('created_at')
        ->first(['id', 'status']);

    expect($run)->not->toBeNull();
    expect((string) $run->status)->toBe('completed');

    $eventTypes = DB::table('run_events')
        ->where('run_id', $run->id)
        ->orderBy('id')
        ->pluck('type')
        ->all();

    expect($eventTypes)->toContain('model_empty_response');
    expect($eventTypes)->toContain('run_completed');
});

test('chat API returns 422 when conversationId is missing', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)->postJson('/api/chat', [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'Hello'],
        ],
    ]);

    $response->assertStatus(422)
        ->assertJsonPath('message', 'conversationId is required (route param or query param)');
});

test('chat API returns 404 when conversation does not belong to user', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)->postJson('/api/chat?conversationId=missing-conversation', [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'Hello'],
        ],
    ]);

    $response->assertNotFound();
});

test('chat API returns 422 when messages are missing or invalid', function () {
    $user = User::factory()->create();

    $conversationId = (string) Str::uuid7();

    DB::table('agent_conversations')->insert([
        'id' => $conversationId,
        'user_id' => $user->id,
        'title' => 'Invalid messages conversation',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    $missingMessages = $this->actingAs($user)->postJson('/api/chat?conversationId='.$conversationId, []);

    $missingMessages->assertStatus(422)
        ->assertJsonPath('message', 'messages must be a non-empty array');

    $invalidMessages = $this->actingAs($user)->postJson('/api/chat?conversationId='.$conversationId, [
        'messages' => [
            ['id' => 'm1', 'role' => 'assistant', 'content' => 'no user prompt'],
        ],
    ]);

    $invalidMessages->assertStatus(422)
        ->assertJsonPath('message', 'A non-empty user message is required');
});

test('guest chat stream can establish session without guest-session preflight', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['Hello from guest stream']);

    $conversationId = 'g-'.str_repeat('b', 32);

    $response = $this->postJson('/api/chat?conversationId='.$conversationId, [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'Hello as guest'],
        ],
    ]);

    $response->assertOk();

    $content = $response->streamedContent();
    expect($content)->toContain('data: {"type":"start"');
    expect($content)->toContain('data: {"type":"finish"');
    expect($content)->toContain("data: [DONE]\n\n");

    /** @var User $guest */
    $guest = User::query()->where('email', 'guest@openagents.internal')->firstOrFail();

    expect(DB::table('agent_conversations')
        ->where('id', $conversationId)
        ->where('user_id', $guest->id)
        ->exists())->toBeTrue();

    expect(DB::table('threads')
        ->where('id', $conversationId)
        ->where('user_id', $guest->id)
        ->exists())->toBeTrue();

    expect(session('chat.guest.conversation_id'))->toBe($conversationId);

    $run = DB::table('runs')
        ->where('thread_id', $conversationId)
        ->where('user_id', $guest->id)
        ->latest('created_at')
        ->first();

    expect($run)->not->toBeNull();

    $toolPolicyApplied = DB::table('run_events')
        ->where('run_id', $run->id)
        ->where('type', 'tool_policy_applied')
        ->first();

    expect($toolPolicyApplied)->not->toBeNull();

    $toolPolicyPayload = json_decode((string) ($toolPolicyApplied->payload ?? ''), true);

    expect($toolPolicyPayload['sessionAuthenticated'] ?? null)->toBeFalse();
    expect($toolPolicyPayload['authRestricted'] ?? null)->toBeTrue();
    expect($toolPolicyPayload['exposedTools'] ?? [])->toBe(['chat_login', 'openagents_api']);
});

test('guest chat stream recovers when request conversation id mismatches established guest session', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['Hello from guest stream']);

    $establishedId = 'g-'.str_repeat('c', 32);
    $mismatchId = 'g-'.str_repeat('d', 32);

    $this->getJson('/api/chat/guest-session?conversationId='.$establishedId)->assertOk();

    $response = $this->postJson('/api/chat?conversationId='.$mismatchId, [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'This should fail'],
        ],
    ]);

    $response->assertOk();

    $content = $response->streamedContent();
    expect($content)->toContain('data: {"type":"start"');
    expect($content)->toContain('data: {"type":"finish"');
    expect($content)->toContain("data: [DONE]\n\n");
});

test('authenticated chat stream adopts guest conversation on demand after chat login', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['Hello after adoption']);

    $user = User::factory()->create();
    $guestService = resolve(GuestChatSessionService::class);

    $conversationId = 'g-'.str_repeat('e', 32);
    $guestService->ensureGuestConversationAndThread($conversationId);

    $response = $this->actingAs($user)->postJson('/api/chat?conversationId='.$conversationId, [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'Continue this chat after login'],
        ],
    ]);

    $response->assertOk();

    $content = $response->streamedContent();
    expect($content)->toContain('data: {"type":"start"');
    expect($content)->toContain('data: {"type":"finish"');
    expect($content)->toContain("data: [DONE]\n\n");

    expect(DB::table('agent_conversations')
        ->where('id', $conversationId)
        ->where('user_id', $user->id)
        ->exists())->toBeTrue();

    expect(DB::table('threads')
        ->where('id', $conversationId)
        ->where('user_id', $user->id)
        ->exists())->toBeTrue();

    $run = DB::table('runs')
        ->where('thread_id', $conversationId)
        ->where('user_id', $user->id)
        ->latest('created_at')
        ->first();

    expect($run)->not->toBeNull();

    expect(DB::table('messages')
        ->where('thread_id', $conversationId)
        ->where('user_id', $user->id)
        ->exists())->toBeTrue();
});

test('authenticated chat stream can start from a fresh guest-style conversation id', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['Hello from a fresh guest-style id']);

    $user = User::factory()->create();
    $conversationId = 'g-'.substr(str_replace('-', '', (string) Str::uuid7()), 0, 32);

    $response = $this->actingAs($user)->postJson('/api/chat?conversationId='.$conversationId, [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'Start from fresh guest id'],
        ],
    ]);

    $response->assertOk();

    $content = $response->streamedContent();
    expect($content)->toContain('data: {"type":"start"');
    expect($content)->toContain('data: {"type":"finish"');
    expect($content)->toContain("data: [DONE]\n\n");

    expect(DB::table('agent_conversations')
        ->where('id', $conversationId)
        ->where('user_id', $user->id)
        ->exists())->toBeTrue();

    expect(DB::table('threads')
        ->where('id', $conversationId)
        ->where('user_id', $user->id)
        ->exists())->toBeTrue();
});

test('guest chat stream succeeds when request conversation id mismatches active guest session', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['Recovered from conversation id mismatch']);

    $active = 'g-'.str_repeat('1', 32);
    $stale = 'g-'.str_repeat('2', 32);

    $this->getJson('/api/chat/guest-session?conversationId='.$active)->assertOk();

    $response = $this->postJson('/api/chat?conversationId='.$stale, [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'Continue despite mismatch'],
        ],
    ]);

    $response->assertOk();
    $content = $response->streamedContent();

    expect($content)->toContain('data: {"type":"start"');
    expect($content)->toContain('data: {"type":"finish"');
    expect($content)->toContain("data: [DONE]\n\n");

    expect(session('chat.guest.conversation_id'))->toBe($active);

    /** @var User $guest */
    $guest = User::query()->where('email', 'guest@openagents.internal')->firstOrFail();

    expect(DB::table('agent_conversations')
        ->where('id', $active)
        ->where('user_id', $guest->id)
        ->exists())->toBeTrue();
});

test('guest chat stream rotates conversation when ownership changes after preflight', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['Recovered after ownership race']);

    $guestService = resolve(GuestChatSessionService::class);
    $owner = User::factory()->create();

    $conversationId = 'g-'.str_repeat('6', 32);

    // Step 1: preflight establishes a valid guest conversation id.
    $guestService->ensureGuestConversationAndThread($conversationId);
    $this->getJson('/api/chat/guest-session?conversationId='.$conversationId)
        ->assertOk()
        ->assertJsonPath('conversationId', $conversationId);

    // Step 2: simulate a race where another flow adopts/claims this id
    // before the next guest stream request executes.
    DB::table('agent_conversations')
        ->where('id', $conversationId)
        ->update([
            'user_id' => $owner->id,
            'updated_at' => now(),
        ]);

    DB::table('threads')
        ->where('id', $conversationId)
        ->update([
            'user_id' => $owner->id,
            'updated_at' => now(),
        ]);

    $response = $this->postJson('/api/chat?conversationId='.$conversationId, [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'recover from ownership race'],
        ],
    ]);

    $response->assertOk();

    $content = $response->streamedContent();
    expect($content)->toContain('data: {"type":"start"');
    expect($content)->toContain('data: {"type":"finish"');
    expect($content)->toContain("data: [DONE]\n\n");

    $rotated = (string) session('chat.guest.conversation_id');

    expect($rotated)->toMatch('/^g-[a-f0-9]{32}$/');
    expect($rotated)->not->toBe($conversationId);

    /** @var User $guest */
    $guest = User::query()->where('email', 'guest@openagents.internal')->firstOrFail();

    expect(DB::table('agent_conversations')
        ->where('id', $rotated)
        ->where('user_id', $guest->id)
        ->exists())->toBeTrue();

    expect(DB::table('runs')
        ->where('thread_id', $rotated)
        ->where('user_id', $guest->id)
        ->exists())->toBeTrue();

    // Original id remains with the non-guest owner.
    expect(DB::table('agent_conversations')
        ->where('id', $conversationId)
        ->where('user_id', $owner->id)
        ->exists())->toBeTrue();
});
