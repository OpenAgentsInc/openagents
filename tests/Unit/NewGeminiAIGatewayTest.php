<?php

declare(strict_types=1);

use App\AI\NewGeminiAIGateway;

test('NewGeminiAIGateway handles gemini responses correctly', function () {
    $prompt = 'What is the capital of France?';
    $inputTokens = 6;
    $answer = 'Capital of France is Paris.';
    $outputTokens = 5;

    $parameters = [
        'model' => 'gemini-1.5-pro-latest',
        'max_tokens' => 30000,
        'messages' => [[
            'role' => 'user',
            'content' => $prompt,
        ]],
        'stream_function' => function ($response) use ($answer) {
            expect($response)->toEqual($answer);
        },
        'stream' => false,
    ];

    $mockResponse = [
        'candidates' => [[
            'content' => [
                'parts' => [[
                    'text' => $answer,
                ]],
            ],
        ]],
        'usageMetadata' => [
            'promptTokenCount' => $inputTokens,
            'candidatesTokenCount' => $outputTokens,
        ],
    ];
    $httpClient = mockGuzzleClient($mockResponse);
    $gateway = new NewGeminiAIGateway($httpClient);

    $result = $gateway->inference($parameters);

    expect($result)->toBeArray();
    expect($result['content'])->toEqual($answer);
    expect($result['input_tokens'])->toEqual($inputTokens);
    expect($result['output_tokens'])->toEqual($outputTokens);
});
