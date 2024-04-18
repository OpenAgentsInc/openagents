<?php

declare(strict_types=1);

use App\AI\PerplexityAIGateway;

test('PerplexityAIGateway handles perplexity responses correctly', function () {
    $prompt = 'What is the capital of France?';
    $inputTokens = 6;
    $answer = 'Capital of France is Paris.';
    $outputTokens = 5;

    $parameters = [
        'model' => 'sonar-small-online',
        'messages' => [
            ['role' => 'user', 'content' => $prompt],
        ],
        'max_tokens' => 2000,
    ];

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
    $gateway = new PerplexityAIGateway($httpClient);

    $result = $gateway->inference($parameters);

    expect($result)->toBeArray();
    expect($result['content'])->toEqual($answer);
    expect($result['input_tokens'])->toEqual($inputTokens);
    expect($result['output_tokens'])->toEqual($outputTokens);
});
