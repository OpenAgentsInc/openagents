<?php

use App\AI\Agents\AutopilotAgent;
use App\Models\Autopilot;
use App\Models\User;
use Illuminate\Support\Facades\DB;
use Laravel\Ai\Ai;
use Laravel\Sanctum\Sanctum;

beforeEach(function () {
    config()->set('posthog.disabled', true);
});

it('manages owned autopilots threads and stream alias through the existing run pipeline', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['Autopilot stream reply']);

    $user = User::factory()->create([
        'email' => 'autopilot-api-user@openagents.com',
    ]);

    $token = $user->createToken('autopilot-api')->plainTextToken;

    $created = $this->withToken($token)
        ->postJson('/api/autopilots', [
            'handle' => 'ep212-bot',
            'displayName' => 'EP212 Bot',
            'status' => 'active',
            'visibility' => 'private',
        ])
        ->assertCreated()
        ->assertJsonPath('data.id', fn (mixed $id): bool => is_string($id) && trim($id) !== '')
        ->assertJsonPath('data.handle', 'ep212-bot');

    $autopilotId = (string) $created->json('data.id');
    expect($autopilotId)->not->toBe('')->and(substr($autopilotId, 14, 1))->toBe('7');

    $this->withToken($token)
        ->getJson('/api/autopilots')
        ->assertOk()
        ->assertJsonFragment(['id' => $autopilotId]);

    $this->withToken($token)
        ->getJson('/api/autopilots/'.$autopilotId)
        ->assertOk()
        ->assertJsonPath('data.handle', 'ep212-bot');

    $this->withToken($token)
        ->getJson('/api/autopilots/ep212-bot')
        ->assertOk()
        ->assertJsonPath('data.id', $autopilotId);

    $this->withToken($token)
        ->patchJson('/api/autopilots/'.$autopilotId, [
            'displayName' => 'EP212 Bot Updated',
            'profile' => [
                'ownerDisplayName' => 'Chris',
                'personaSummary' => 'Pragmatic and concise',
                'autopilotVoice' => 'calm and direct',
            ],
            'policy' => [
                'toolAllowlist' => ['openagents_api', 'lightning_l402_fetch'],
                'toolDenylist' => ['lightning_l402_fetch'],
                'l402RequireApproval' => true,
                'l402MaxSpendMsatsPerCall' => 100000,
                'l402AllowedHosts' => ['sats4ai.com'],
            ],
        ])
        ->assertOk()
        ->assertJsonPath('data.displayName', 'EP212 Bot Updated')
        ->assertJsonPath('data.configVersion', 2);

    $threadCreated = $this->withToken($token)
        ->postJson('/api/autopilots/'.$autopilotId.'/threads', [
            'title' => 'Autopilot test thread',
        ])
        ->assertCreated()
        ->assertJsonPath('data.autopilotId', $autopilotId);

    $threadId = (string) $threadCreated->json('data.id');
    expect($threadId)->not->toBe('');

    $autopilotIdOnThread = DB::table('threads')
        ->where('id', $threadId)
        ->value('autopilot_id');

    expect($autopilotIdOnThread)->toBe($autopilotId);

    $this->withToken($token)
        ->getJson('/api/autopilots/'.$autopilotId.'/threads')
        ->assertOk()
        ->assertJsonFragment([
            'id' => $threadId,
            'autopilotId' => $autopilotId,
        ]);

    $streamResponse = $this->withToken($token)
        ->postJson('/api/autopilots/'.$autopilotId.'/stream', [
            'conversationId' => $threadId,
            'messages' => [
                ['id' => 'm1', 'role' => 'user', 'content' => 'hello from autopilot stream alias'],
            ],
        ]);

    $streamResponse->assertOk();
    $streamed = $streamResponse->streamedContent();

    AutopilotAgent::assertPrompted(function ($prompt): bool {
        $instructions = (string) $prompt->agent->instructions();

        return str_contains($instructions, 'Runtime Autopilot profile context (private):')
            && str_contains($instructions, 'owner_display_name=Chris')
            && str_contains($instructions, 'persona_summary=Pragmatic and concise')
            && str_contains($instructions, 'autopilot_voice=calm and direct');
    });
    expect($streamed)->toContain('data: {"type":"start"');
    expect($streamed)->toContain('data: {"type":"finish"');
    expect($streamed)->toContain("data: [DONE]\n\n");

    $run = DB::table('runs')
        ->where('thread_id', $threadId)
        ->where('user_id', $user->id)
        ->orderByDesc('created_at')
        ->first(['id', 'autopilot_id', 'autopilot_config_version']);

    expect($run)->not->toBeNull();
    expect($run->autopilot_id)->toBe($autopilotId);
    expect((int) $run->autopilot_config_version)->toBe(2);

    $runMessages = DB::table('messages')
        ->where('run_id', $run->id)
        ->where('thread_id', $threadId)
        ->where('user_id', $user->id)
        ->orderBy('created_at')
        ->get(['role', 'autopilot_id']);

    expect($runMessages)->toHaveCount(2);
    expect($runMessages->pluck('autopilot_id')->unique()->values()->all())->toBe([$autopilotId]);

    $runEvents = DB::table('run_events')
        ->where('run_id', $run->id)
        ->where('thread_id', $threadId)
        ->where('user_id', $user->id)
        ->orderBy('id')
        ->get(['type', 'autopilot_id', 'actor_type', 'actor_autopilot_id', 'payload']);

    expect($runEvents)->not->toBeEmpty();
    expect($runEvents->pluck('autopilot_id')->unique()->values()->all())->toBe([$autopilotId]);

    $runStarted = $runEvents->firstWhere('type', 'run_started');
    expect($runStarted)->not->toBeNull();
    expect($runStarted->actor_type)->toBe('user');
    expect($runStarted->actor_autopilot_id)->toBeNull();

    $toolPolicyApplied = $runEvents->firstWhere('type', 'tool_policy_applied');
    expect($toolPolicyApplied)->not->toBeNull();

    $toolPolicyPayload = json_decode((string) ($toolPolicyApplied->payload ?? ''), true);
    expect($toolPolicyPayload)->toBeArray();
    expect($toolPolicyPayload['policyApplied'] ?? null)->toBeTrue();
    expect($toolPolicyPayload['exposedTools'] ?? null)->toBe(['openagents_api']);
    expect($toolPolicyPayload['removedByDenylist'] ?? [])->toContain('lightning_l402_fetch');
    expect($toolPolicyPayload['removedByAllowlist'] ?? [])->toContain('lightning_l402_approve');

    $modelStarted = $runEvents->firstWhere('type', 'model_stream_started');
    expect($modelStarted)->not->toBeNull();
    expect($modelStarted->actor_type)->toBe('autopilot');
    expect($modelStarted->actor_autopilot_id)->toBe($autopilotId);

    $runCompleted = $runEvents->firstWhere('type', 'run_completed');
    expect($runCompleted)->not->toBeNull();
    expect($runCompleted->actor_type)->toBe('autopilot');
    expect($runCompleted->actor_autopilot_id)->toBe($autopilotId);

    $threadCountBefore = DB::table('threads')
        ->where('user_id', $user->id)
        ->where('autopilot_id', $autopilotId)
        ->count();

    $this->withToken($token)
        ->postJson('/api/autopilots/'.$autopilotId.'/stream', [
            'messages' => [
                ['id' => 'm2', 'role' => 'user', 'content' => 'create a new thread implicitly'],
            ],
        ])
        ->assertOk();

    $threadCountAfter = DB::table('threads')
        ->where('user_id', $user->id)
        ->where('autopilot_id', $autopilotId)
        ->count();

    expect($threadCountAfter)->toBe($threadCountBefore + 1);

    $autopilotRunCount = DB::table('runs')
        ->where('user_id', $user->id)
        ->where('autopilot_id', $autopilotId)
        ->count();

    expect($autopilotRunCount)->toBeGreaterThanOrEqual(2);
});

it('enforces autopilot ownership for read write thread and stream routes', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['ownership check']);

    $owner = User::factory()->create([
        'email' => 'autopilot-owner@openagents.com',
    ]);
    $intruder = User::factory()->create([
        'email' => 'autopilot-intruder@openagents.com',
    ]);

    Sanctum::actingAs($owner);

    $created = $this->postJson('/api/autopilots', [
        'handle' => 'owner-bot',
        'displayName' => 'Owner Bot',
    ])
        ->assertCreated()
        ->assertJsonPath('data.id', fn (mixed $id): bool => is_string($id) && trim($id) !== '');

    $autopilotId = (string) $created->json('data.id');

    Sanctum::actingAs($intruder);

    $this->getJson('/api/autopilots/'.$autopilotId)
        ->assertNotFound();

    $this->patchJson('/api/autopilots/'.$autopilotId, [
        'displayName' => 'Hacked',
    ])
        ->assertNotFound();

    $this->getJson('/api/autopilots/owner-bot')
        ->assertNotFound();

    $this->getJson('/api/autopilots/'.$autopilotId.'/threads')
        ->assertNotFound();

    $this->postJson('/api/autopilots/'.$autopilotId.'/threads', [
        'title' => 'intruder thread',
    ])
        ->assertNotFound();

    $this->postJson('/api/autopilots/'.$autopilotId.'/stream', [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'intruder stream'],
        ],
    ])
        ->assertNotFound();

    expect(Autopilot::query()->where('owner_user_id', $owner->id)->where('id', $autopilotId)->exists())->toBeTrue();
    expect(Autopilot::query()->where('owner_user_id', $intruder->id)->where('id', $autopilotId)->exists())->toBeFalse();
});
