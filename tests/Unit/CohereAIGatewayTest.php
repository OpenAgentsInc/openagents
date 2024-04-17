<?php
declare(strict_types=1);

use App\AI\CohereAIGateway;
use GuzzleHttp\Client;
use GuzzleHttp\Psr7\Response;
use GuzzleHttp\Handler\MockHandler;
use GuzzleHttp\HandlerStack;

test('CohereAIGateway handles mistral responses correctly', function () {
    $prompt = 'What is the capital of France?';
    $inputTokens = 6;
    $answer = 'Capital of France is Paris.';
    $outputTokens = 5;

    $parameters = [
        'message' => $prompt
    ];

    $mockResponse = [
        'text' => $answer,
        'meta' => [
            'tokens' => [
                'input_tokens' => $inputTokens,
                'output_tokens' => $outputTokens
            ]
        ]
    ];
    $mockResponseStream = fopen('php://memory', 'r+');
    fwrite(
        $mockResponseStream,
        json_encode($mockResponse) . "\n"
    );
    rewind($mockResponseStream);
    
    $mock = new MockHandler([
        new Response(200, [], $mockResponseStream)
    ]);
    
    $handlerStack = HandlerStack::create($mock);
    $httpClient = new Client(['handler' => $handlerStack]);
    
    $gateway = new CohereAIGateway($httpClient);
    
    $result = $gateway->inference($parameters);

    expect($result)->toBeArray();
    expect($result['content'])->toEqual($answer);
    expect($result['input_tokens'])->toEqual($inputTokens);
    expect($result['output_tokens'])->toEqual($outputTokens);
});
