<?php

use App\AI\SimpleInferencer;
use App\Models\Thread;
use App\Models\User;

test('LLM inference saves tokens to Message model', function () {
    // Arrange
    $user = User::factory()->create();
    $thread = Thread::factory()->create(['user_id' => $user->id]);
    $thread->messages()->create([
        'body' => 'Hello, world!',
    ]);
    $prompt = 'What is the capital of France?';
    $model = 'claude-3-sonnet-20240229';

    // Act
    $streamFunction = function ($chunk) {
    };
    $response = SimpleInferencer::inference($prompt, $model, $thread, $streamFunction);
    dd($response); // "Hello there! It's nice to meet you. I'm an AI assistant created by Anthropic. How can I be of help to you today?"
});
