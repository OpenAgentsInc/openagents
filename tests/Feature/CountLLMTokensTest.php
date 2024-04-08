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
    $model = 'gpt-3.5-turbo-16k';
    //    $model = 'claude-3-sonnet-20240229';

    // Act
    $streamFunction = function ($chunk) {
    };
    $response = SimpleInferencer::inference($prompt, $model, $thread, $streamFunction);

})->skip();
