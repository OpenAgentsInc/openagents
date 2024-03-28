<?php

use App\AI\GeminiAIGateway;

test('can hit gemini endpoint', function () {
    $gemini = new GeminiAIGateway();
    $inference = $gemini->inference('Hello, world!');
    expect($inference)->toBeArray();
});
