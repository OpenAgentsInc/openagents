<?php

use App\AI\PoolRag;
use App\Models\Thread;
use App\Models\User;

it('sets the conversation history for the given Thread', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create(['user_id' => $user->id]);

    $oldPrompt = 'What is the capital of France?';
    $thread->messages()->create([
        'body' => $oldPrompt,
    ]);

    $oldAnswer = 'Capital of France is Paris.';
    $thread->messages()->create([
        'model' => 'command-r-plus',
        'body' => $oldAnswer,
    ]);

    $newPrompt = 'What is the capital of Spain?';
    $thread->messages()->create([
        'body' => $newPrompt,
    ]);

    $messages = [
        ['role' => 'user', 'content' => $oldPrompt],
        ['role' => 'assistant', 'content' => $oldAnswer],
        ['role' => 'user', 'content' => $newPrompt],
    ];

    $nostrRag = (new PoolRag())->history($thread, 100);

    expect($nostrRag->getMessages())->toBe($messages);

    // If messages do not fit into maxTokens, lose oldest message first
    array_shift($messages);
    $nostrRag = (new PoolRag())->history($thread, 13);

    expect($nostrRag->getMessages())->toBe($messages);
});
