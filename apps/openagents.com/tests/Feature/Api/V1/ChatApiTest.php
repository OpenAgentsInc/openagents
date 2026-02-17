<?php

use App\AI\Agents\AutopilotAgent;
use App\Models\User;
use Illuminate\Support\Facades\DB;
use Laravel\Ai\Ai;

beforeEach(function () {
    config()->set('posthog.disabled', true);
});

it('supports creating and reading chats through api', function () {
    Ai::fakeAgent(AutopilotAgent::class, ['Hello from API']);

    $user = User::factory()->create([
        'email' => 'chat-user@openagents.com',
    ]);

    $token = $user->createToken('chat-api')->plainTextToken;

    $createResponse = $this->withToken($token)
        ->postJson('/api/chats', [
            'title' => 'API Chat',
        ]);

    $createResponse->assertCreated()
        ->assertJsonPath('data.title', 'API Chat');

    $conversationId = $createResponse->json('data.id');
    expect($conversationId)->toBeString()->not->toBeEmpty();

    $streamResponse = $this->withToken($token)
        ->postJson('/api/chats/'.$conversationId.'/stream', [
            'messages' => [
                ['id' => 'm1', 'role' => 'user', 'content' => 'Say hi from API'],
            ],
        ]);

    $streamResponse->assertOk();

    $streamed = $streamResponse->streamedContent();
    expect($streamed)->toContain('data: {"type":"start"');
    expect($streamed)->toContain('data: {"type":"finish"');
    expect($streamed)->toContain("data: [DONE]\n\n");

    $aliasStreamResponse = $this->withToken($token)
        ->postJson('/api/chat/stream?conversationId='.$conversationId, [
            'conversationId' => $conversationId,
            'messages' => [
                ['id' => 'm2', 'role' => 'user', 'content' => 'Say hi from alias stream'],
            ],
        ]);

    $aliasStreamResponse->assertOk();

    $aliasStreamed = $aliasStreamResponse->streamedContent();
    expect($aliasStreamed)->toContain('data: {"type":"start"');
    expect($aliasStreamed)->toContain('data: {"type":"finish"');
    expect($aliasStreamed)->toContain("data: [DONE]\n\n");

    $this->withToken($token)
        ->getJson('/api/chats')
        ->assertOk()
        ->assertJsonFragment(['id' => $conversationId]);

    $showResponse = $this->withToken($token)
        ->getJson('/api/chats/'.$conversationId)
        ->assertOk();

    $showResponse->assertJsonPath('data.conversation.id', $conversationId);

    $messagesResponse = $this->withToken($token)
        ->getJson('/api/chats/'.$conversationId.'/messages')
        ->assertOk();

    $messages = collect($messagesResponse->json('data'));
    expect($messages->pluck('role')->all())->toContain('user');
    expect($messages->pluck('role')->all())->toContain('assistant');

    $runsResponse = $this->withToken($token)
        ->getJson('/api/chats/'.$conversationId.'/runs')
        ->assertOk();

    $runs = collect($runsResponse->json('data'));
    expect($runs)->not->toBeEmpty();

    $runId = (string) $runs->first()['id'];

    $this->withToken($token)
        ->getJson('/api/chats/'.$conversationId.'/runs/'.$runId.'/events')
        ->assertOk()
        ->assertJsonPath('data.run.id', $runId);

    expect(DB::table('threads')->where('id', $conversationId)->where('user_id', $user->id)->exists())->toBeTrue();
});
