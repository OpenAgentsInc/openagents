<?php

use App\AI\BedrockAIGateway;
use App\Services\ToolService;

test('converse stream works', function () {

    $service = new ToolService();
    $gateway = new BedrockAIGateway($service);

    dd($gateway);
});
