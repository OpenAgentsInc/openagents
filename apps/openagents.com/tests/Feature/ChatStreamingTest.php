<?php

use App\AI\Agents\AutopilotAgent;
use App\Models\User;
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

    $rows = DB::table('agent_conversation_messages')
        ->where('conversation_id', $conversationId)
        ->orderBy('created_at')
        ->get(['role', 'content']);

    expect($rows)->toHaveCount(2);
    expect($rows[0]->role)->toBe('user');
    expect($rows[1]->role)->toBe('assistant');
    expect($rows[1]->content)->toContain('Hello from fake agent');
});
