<?php

use App\AI\BedrockAIGateway;
use App\Services\ToolService;
use Illuminate\Support\Facades\Log;

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

    foreach ($result['stream'] as $event) {
        if (isset($event['contentBlockDelta'])) {
            // Handle the 'contentBlockDelta' event.
        } else if (isset($event['contentBlockStart'])) {
            // Handle the 'contentBlockStart' event.
        } else if (isset($event['contentBlockStop'])) {
            // Handle the 'contentBlockStop' event.
        } else if (isset($event['internalServerException'])) {
            // Handle the 'internalServerException' event.
        } else if (isset($event['messageStart'])) {
            // Handle the 'messageStart' event.
        } else if (isset($event['messageStop'])) {
            // Handle the 'messageStop' event.
        } else if (isset($event['metadata'])) {
            // Handle the 'metadata' event.
        } else if (isset($event['modelStreamErrorException'])) {
            // Handle the 'modelStreamErrorException' event.
        } else if (isset($event['serviceUnavailableException'])) {
            // Handle the 'serviceUnavailableException' event.
        } else if (isset($event['throttlingException'])) {
            // Handle the 'throttlingException' event.
        } else if (isset($event['validationException'])) {
            // Handle the 'validationException' event.
        }
    }
});
