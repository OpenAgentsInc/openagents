<?php

declare(strict_types=1);

use App\AI\CohereAIGateway;

test('CohereAIGateway handles cohere responses correctly', function () {
    $prompt = 'What is the capital of France?';
    $inputTokens = 6;
    $answer = 'Capital of France is Paris.';
    $outputTokens = 5;

    $parameters = [
        'message' => $prompt,
        'stream' => false,
    ];

    $mockResponse = [
        'text' => $answer,
        'meta' => [
            'tokens' => [
                'input_tokens' => $inputTokens,
                'output_tokens' => $outputTokens,
            ],
        ],
    ];
    $httpClient = mockGuzzleClient($mockResponse);
    $gateway = new CohereAIGateway($httpClient);

    $result = $gateway->inference($parameters);

    expect($result)->toBeArray();
    expect($result['content'])->toEqual($answer);
    expect($result['input_tokens'])->toEqual($inputTokens);
    expect($result['output_tokens'])->toEqual($outputTokens);
});
