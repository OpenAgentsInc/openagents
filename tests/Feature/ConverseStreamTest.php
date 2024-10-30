<?php

use App\AI\BedrockAIGateway;
use App\Services\ToolService;

test('converse stream works', function () {
    $service = new ToolService();
    $gateway = new BedrockAIGateway($service);

    $result = $gateway->converseStream([
        'messages' => [
            [
                'role' => 'user',
                'content' => 'Say hello world.'
            ]
        ]
    ]);

    $receivedMessageStart = false;
    $receivedContentDelta = false;
    $receivedContentStop = false;
    $receivedMessageStop = false;
    $receivedMetadata = false;
    $fullText = '';

    foreach ($result['stream'] as $event) {
        if (isset($event['messageStart'])) {
            expect($event['messageStart'])->toHaveKey('role');
            expect($event['messageStart']['role'])->toBe('assistant');
            $receivedMessageStart = true;
        } else if (isset($event['contentBlockDelta'])) {
            expect($event['contentBlockDelta'])->toHaveKey('delta');
            expect($event['contentBlockDelta']['delta'])->toHaveKey('text');
            expect($event['contentBlockDelta'])->toHaveKey('contentBlockIndex');
            $fullText .= $event['contentBlockDelta']['delta']['text'];
            $receivedContentDelta = true;
        } else if (isset($event['contentBlockStop'])) {
            expect($event['contentBlockStop'])->toHaveKey('contentBlockIndex');
            $receivedContentStop = true;
        } else if (isset($event['messageStop'])) {
            expect($event['messageStop'])->toHaveKey('stopReason');
            expect($event['messageStop']['stopReason'])->toBe('end_turn');
            $receivedMessageStop = true;
        } else if (isset($event['metadata'])) {
            expect($event['metadata'])->toHaveKey('usage');
            expect($event['metadata']['usage'])->toHaveKeys(['inputTokens', 'outputTokens', 'totalTokens']);
            expect($event['metadata'])->toHaveKey('metrics');
            expect($event['metadata']['metrics'])->toHaveKey('latencyMs');
            $receivedMetadata = true;
        }
    }

    // Assert we received all expected event types
    expect($receivedMessageStart)->toBeTrue('Did not receive messageStart event');
    expect($receivedContentDelta)->toBeTrue('Did not receive contentBlockDelta event');
    expect($receivedContentStop)->toBeTrue('Did not receive contentBlockStop event');
    expect($receivedMessageStop)->toBeTrue('Did not receive messageStop event');
    expect($receivedMetadata)->toBeTrue('Did not receive metadata event');

    // Assert the full text contains something
    expect($fullText)->not->toBeEmpty();
    expect($fullText)->toContain('Hello');
})->skip('long, skipping for now');
