<?php

declare(strict_types=1);

use App\AI\TogetherAIGateway;

test('TogetherAIGateway handles llama responses correctly', function () {
    $prompt = 'What is the capital of France?';
    $inputTokens = 6;
    $answer = 'Capital of France is Paris.';
    $outputTokens = 5;

    $parameters = [
        'model' => 'meta-llama/llama-3-8b-chat-hf',
        'messages' => [[
            'role' => 'user',
            'content' => $prompt,
            'foo' => 'bar',
        ]],
        'stream_function' => function ($response) use ($answer) {
            expect($response)->toEqual($answer);
        },
        'stream' => false,
    ];

    $mockResponse = [
        'choices' => [[
            'message' => [
                'content' => $answer,
            ],
        ]],
        'usage' => [
            'prompt_tokens' => $inputTokens,
            'completion_tokens' => $outputTokens,
        ],
    ];
    $httpClient = mockGuzzleClient($mockResponse);
    $gateway = new TogetherAIGateway($httpClient);

    $result = $gateway->inference($parameters);

    expect($result)->toBeArray();
    expect($result['content'])->toEqual($answer);
    expect($result['input_tokens'])->toEqual($inputTokens);
    expect($result['output_tokens'])->toEqual($outputTokens);
});
