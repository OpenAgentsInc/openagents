<?php
declare(strict_types=1);

use App\AI\PerplexityAIGateway;
use GuzzleHttp\Client;
use GuzzleHttp\Psr7\Response;
use GuzzleHttp\Handler\MockHandler;
use GuzzleHttp\HandlerStack;

test('PerplexityAIGateway handles mistral responses correctly', function () {
    $prompt = 'What is the capital of France?';
    $inputTokens = 6;
    $answer = 'Capital of France is Paris.';
    $outputTokens = 5;

    $parameters = [
        'model' => 'sonar-small-online',
        'messages' => [
            ['role' => 'user', 'content' => $prompt]
        ],
        'max_tokens' => 2000
    ];

    $mockResponse = [
        [
            'choices' => [[
                'delta' => [
                    'content' => $answer
                ]
            ]],
            'usage' => [
                'prompt_tokens' => $inputTokens,
                'completion_tokens' => $outputTokens
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
    
    $gateway = new PerplexityAIGateway($httpClient);
    
    $result = $gateway->inference($parameters);

    expect($result)->toBeArray();
    expect($result['content'])->toEqual($answer);
    expect($result['input_tokens'])->toEqual($inputTokens);
    expect($result['output_tokens'])->toEqual($outputTokens);
});
