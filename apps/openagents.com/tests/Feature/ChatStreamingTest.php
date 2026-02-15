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

    $msgs = DB::table('messages')->where('thread_id', $conversationId)->where('user_id', $user->id)->orderBy('created_at')->get(['role', 'content']);
    expect($msgs)->toHaveCount(2);
    expect($msgs[0]->role)->toBe('user');
    expect($msgs[1]->role)->toBe('assistant');
    expect($msgs[1]->content)->toContain('Hello from fake agent');

    $events = DB::table('run_events')->where('run_id', $run->id)->where('user_id', $user->id)->orderBy('id')->pluck('type')->all();
    expect($events)->toContain('run_started');
    expect($events)->toContain('model_stream_started');
    expect($events)->toContain('model_finished');
    expect($events)->toContain('run_completed');
});
