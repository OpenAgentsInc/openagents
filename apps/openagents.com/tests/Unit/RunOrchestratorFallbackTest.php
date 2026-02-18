<?php

use App\AI\RunOrchestrator;

test('run orchestrator builds shout fallback from openagents api tool result', function () {
    $orchestrator = resolve(RunOrchestrator::class);

    $method = new ReflectionMethod($orchestrator, 'buildEmptyResponseFallback');
    $method->setAccessible(true);

    $text = $method->invoke($orchestrator, [
        'tool_name' => 'openagents_api',
        'successful' => true,
        'error' => null,
        'result' => [
            'status' => 'ok',
            'action' => 'request',
            'path' => '/api/shouts',
            'method' => 'POST',
            'response' => [
                'json' => [
                    'data' => ['id' => 123],
                ],
            ],
        ],
    ], 'stop');

    expect($text)->toBe('Done. I posted that shout to the feed.');
});

test('run orchestrator keeps generic fallback when no tool result exists', function () {
    $orchestrator = resolve(RunOrchestrator::class);

    $method = new ReflectionMethod($orchestrator, 'buildEmptyResponseFallback');
    $method->setAccessible(true);

    $text = $method->invoke($orchestrator, null, 'stop');

    expect($text)->toBe("I couldn't generate a response from the model. Please try again.");
});
