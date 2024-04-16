<?php

use App\AI\AnthropicAIGateway;
use GuzzleHttp\Client;
use GuzzleHttp\Psr7\Response;
use GuzzleHttp\Handler\MockHandler;
use GuzzleHttp\HandlerStack;

test('AnthropicAIGateway createStreamed handles stream correctly', function () {
    $prompt = 'What is the capital of France?';
    $inputTokens = 6;
    $answer = 'Capital of France is Paris.';
    $outputTokens = 5;

    $parameters = [
        'model' => 'claude-3-haiku-20240307',
        'messages' => [
            ['role' => 'user', 'content' => $prompt]
        ],
        'max_tokens' => 4096
    ];

    $mockResponse = [
        [
            'type' => 'content_block_delta',
            'delta' => [
                'text' => $answer
            ]
        ],
        [
            'type' => 'message_start',
            'message' => [
                'usage' => [
                    'input_tokens' => $inputTokens
                ]
            ]
        ],
        [
            'type' => 'message_delta',
            'usage' => [
                'output_tokens' => $outputTokens
            ]
        ]
    ];
    $mockResponse = array_map(function($data) {
        return 'data: ' . json_encode($data);
    }, $mockResponse);
    $mockResponseStream = fopen('php://memory', 'r+');
    fwrite(
        $mockResponseStream,
        \implode("\n", $mockResponse) . "\n"
    );
    rewind($mockResponseStream);
    
    $mock = new MockHandler([
        new Response(200, [], $mockResponseStream)
    ]);
    
    $handlerStack = HandlerStack::create($mock);
    $httpClient = new Client(['handler' => $handlerStack]);
    
    $gateway = new AnthropicAIGateway($httpClient);
    
    $result = $gateway->inference($parameters);

    expect($result)->toBeArray();
    expect($result['content'])->toEqual($answer);
    expect($result['input_tokens'])->toEqual($inputTokens);
    expect($result['output_tokens'])->toEqual($outputTokens);
});

