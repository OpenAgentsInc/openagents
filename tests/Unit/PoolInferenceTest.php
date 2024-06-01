<?php

declare(strict_types=1);

use App\AI\PoolInference;
use App\Models\Agent;
use App\Models\PoolJob;
use App\Models\Thread;

test('PoolInferencer can inference', function () {
    $thread = Thread::factory()->create();
    $agent = Agent::factory()->create([
        'prompt' => 'You are an AI agent',
    ]);
    $job = PoolJob::factory()->create([
        'agent_id' => $agent->id,
        'thread_id' => $thread->id,
        'content' => 'Matching RAG context.',
    ]);

    $prompt = 'What is the capital of France?';
    $inputTokens = 6;
    $answer = 'Capital of France is Paris.';
    $outputTokens = 5;

    $thread->messages()->create([
        'body' => $prompt,
    ]);

    $streamFunction = function ($chunk) use ($answer) {
        $this->assertEquals($answer, $chunk);
    };

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
    $requestContainer = [];
    $httpClient = mockGuzzleClient($mockResponse, $requestContainer);

    $inference = new PoolInference($httpClient);
    $result = $inference->inference('sonar-small-online', $job, $streamFunction);

    $payload = json_decode($requestContainer[0]->getBody()->getContents(), true);
    $this->assertEquals([
        'model' => 'sonar-small-online',
        'messages' => [
            [
                'role' => 'system',
                'content' => $agent->prompt."\n".
                    "You can use the following extracted parts of a long document to help you answer the user's questions.\n".
                    $job->content,
            ],
            [
                'role' => 'user',
                'content' => $prompt,
            ],
        ],
        'stream' => true,
        'max_tokens' => 1962,
    ], $payload);

    expect($result)->toBeArray();
    expect($result['content'])->toEqual($answer);
    expect($result['input_tokens'])->toEqual($inputTokens);
    expect($result['output_tokens'])->toEqual($outputTokens);
});
