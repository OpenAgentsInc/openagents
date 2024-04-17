<?php

declare(strict_types=1);

use App\AI\AnthropicAIGateway;

test('AnthropicAIGateway handles Claude responses correctly', function () {
    $prompt = 'What is the capital of France?';
    $inputTokens = 6;
    $answer = 'Capital of France is Paris.';
    $outputTokens = 5;

    $parameters = [
        'model' => 'claude-3-haiku-20240307',
        'messages' => [
            ['role' => 'user', 'content' => $prompt],
        ],
        'max_tokens' => 4096,
    ];

    $mockResponse = [
        [
            'type' => 'content_block_delta',
            'delta' => [
                'text' => $answer,
            ],
        ],
        [
            'type' => 'message_start',
            'message' => [
                'usage' => [
                    'input_tokens' => $inputTokens,
                ],
            ],
        ],
        [
            'type' => 'message_delta',
            'usage' => [
                'output_tokens' => $outputTokens,
            ],
        ],
    ];
    $httpClient = mockGuzzleClient($mockResponse);
    $gateway = new AnthropicAIGateway($httpClient);

    $result = $gateway->inference($parameters);

    expect($result)->toBeArray();
    expect($result['content'])->toEqual($answer);
    expect($result['input_tokens'])->toEqual($inputTokens);
    expect($result['output_tokens'])->toEqual($outputTokens);
});
