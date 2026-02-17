<?php

use App\AI\Agents\AutopilotAgent;
use App\Models\User;
use Laravel\Ai\Ai;

beforeEach(function () {
    config()->set('posthog.disabled', true);
});

it('supports autopilot phase a api routes and stream alias', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['Autopilot stream reply']);

    $user = User::factory()->create([
        'email' => 'autopilot-api-user@openagents.com',
    ]);

    $token = $user->createToken('autopilot-api')->plainTextToken;

    $this->withToken($token)
        ->getJson('/api/autopilots')
        ->assertOk()
        ->assertJsonPath('data.0.id', 'default')
        ->assertJsonPath('data.0.ownerUserId', $user->id);

    $this->withToken($token)
        ->postJson('/api/autopilots', [
            'handle' => 'default',
            'displayName' => 'Autopilot',
        ])
        ->assertCreated()
        ->assertJsonPath('data.id', 'default');

    $this->withToken($token)
        ->getJson('/api/autopilots/default')
        ->assertOk()
        ->assertJsonPath('data.handle', 'default');

    $this->withToken($token)
        ->patchJson('/api/autopilots/default', [
            'displayName' => 'Autopilot',
            'status' => 'active',
            'visibility' => 'private',
        ])
        ->assertOk()
        ->assertJsonPath('data.id', 'default');

    $threadCreate = $this->withToken($token)
        ->postJson('/api/autopilots/default/threads', [
            'title' => 'Autopilot test thread',
        ])
        ->assertCreated();

    $threadId = $threadCreate->json('data.id');
    expect($threadId)->toBeString()->not->toBeEmpty();

    $this->withToken($token)
        ->getJson('/api/autopilots/default/threads')
        ->assertOk()
        ->assertJsonFragment([
            'id' => $threadId,
            'autopilotId' => 'default',
        ]);

    $streamResponse = $this->withToken($token)
        ->postJson('/api/autopilots/default/stream', [
            'conversationId' => $threadId,
            'messages' => [
                ['id' => 'm1', 'role' => 'user', 'content' => 'hello from autopilot stream alias'],
            ],
        ]);

    $streamResponse->assertOk();

    $streamed = $streamResponse->streamedContent();
    expect($streamed)->toContain('data: {"type":"start"');
    expect($streamed)->toContain('data: {"type":"finish"');
    expect($streamed)->toContain("data: [DONE]\n\n");
});

it('returns not found for unsupported autopilot id', function () {
    $user = User::factory()->create([
        'email' => 'autopilot-not-found@openagents.com',
    ]);

    $token = $user->createToken('autopilot-api')->plainTextToken;

    $this->withToken($token)
        ->getJson('/api/autopilots/nonexistent')
        ->assertNotFound();

    $this->withToken($token)
        ->postJson('/api/autopilots/nonexistent/stream', [
            'messages' => [
                ['id' => 'm1', 'role' => 'user', 'content' => 'hi'],
            ],
        ])
        ->assertNotFound();
});
