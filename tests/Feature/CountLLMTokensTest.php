<?php

use App\AI\SimpleInferencer;
use App\Models\Thread;
use App\Models\User;

test('LLM inference counts tokens for the Message model', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create(['user_id' => $user->id]);
    $thread->messages()->create([
        'body' => 'Hello, world!',
    ]);
    $prompt = 'What is the capital of France?';
    $inputTokens = 6;
    $answer = 'Capital of France is Paris.';
    $outputTokens = 5;
    $model = 'mistral-small-latest';
    $mockResponse = [
        [
            'choices' => [[
                'delta' => [
                    'content' => $answer,
                ],
            ]],
            'usage' => [
                'prompt_tokens' => $inputTokens,
                'completion_tokens' => $outputTokens,
            ],
        ],
    ];
    $httpClient = mockGuzzleClient($mockResponse);

    $streamFunction = function ($chunk) {
        $this->assertIsString($chunk);
    };
    $result = SimpleInferencer::inference($prompt, $model, $thread, $streamFunction, $httpClient);

    expect($result)->toBeArray();
    expect($result['content'])->toEqual($answer);
    expect($result['input_tokens'])->toEqual($inputTokens);
    expect($result['output_tokens'])->toEqual($outputTokens);
});
