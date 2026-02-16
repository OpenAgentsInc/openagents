<?php

use App\AI\Agents\AutopilotAgent;
use App\Models\User;
use Illuminate\Support\Facades\DB;
use Laravel\Ai\Ai;

beforeEach(function () {
    config()->set('posthog.disabled', true);
});

it('supports creating and reading chats through api v1', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['Hello from API v1']);

    $user = User::factory()->create([
        'email' => 'chat-user@openagents.com',
    ]);

    $token = $user->createToken('chat-api')->plainTextToken;

    $createResponse = $this->withToken($token)
        ->postJson('/api/v1/chats', [
            'title' => 'API Chat',
        ]);

    $createResponse->assertCreated()
        ->assertJsonPath('data.title', 'API Chat');

    $conversationId = $createResponse->json('data.id');
    expect($conversationId)->toBeString()->not->toBeEmpty();

    $streamResponse = $this->withToken($token)
        ->postJson('/api/v1/chats/'.$conversationId.'/stream', [
            'messages' => [
                ['id' => 'm1', 'role' => 'user', 'content' => 'Say hi from API'],
            ],
        ]);

    $streamResponse->assertOk();

    $streamed = $streamResponse->streamedContent();
    expect($streamed)->toContain('data: {"type":"start"');
    expect($streamed)->toContain('data: {"type":"finish"');
    expect($streamed)->toContain("data: [DONE]\n\n");

    $this->withToken($token)
        ->getJson('/api/v1/chats')
        ->assertOk()
        ->assertJsonFragment(['id' => $conversationId]);

    $showResponse = $this->withToken($token)
        ->getJson('/api/v1/chats/'.$conversationId)
        ->assertOk();

    $showResponse->assertJsonPath('data.conversation.id', $conversationId);

    $messagesResponse = $this->withToken($token)
        ->getJson('/api/v1/chats/'.$conversationId.'/messages')
        ->assertOk();

    $messages = collect($messagesResponse->json('data'));
    expect($messages->pluck('role')->all())->toContain('user');
    expect($messages->pluck('role')->all())->toContain('assistant');

    $runsResponse = $this->withToken($token)
        ->getJson('/api/v1/chats/'.$conversationId.'/runs')
        ->assertOk();

    $runs = collect($runsResponse->json('data'));
    expect($runs)->not->toBeEmpty();

    $runId = (string) $runs->first()['id'];

    $this->withToken($token)
        ->getJson('/api/v1/chats/'.$conversationId.'/runs/'.$runId.'/events')
        ->assertOk()
        ->assertJsonPath('data.run.id', $runId);

    expect(DB::table('threads')->where('id', $conversationId)->where('user_id', $user->id)->exists())->toBeTrue();
});
