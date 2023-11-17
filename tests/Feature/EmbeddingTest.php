<?php

use App\Services\QueenbeeGateway;

test('can create embedding from text', function () {

    $gateway = new QueenbeeGateway();
    $result = $gateway->createEmbedding("What is an AI agent?");
    $embedding = $result[0]['embedding'];

    // Check if the array contains exactly 768 elements
    expect(count($embedding))->toBe(768);

    // Check if elements are numeric
    expect(is_numeric($embedding[0]))->toBeTrue();
    expect(is_numeric($embedding[1]))->toBeTrue();
    expect(is_numeric($embedding[2]))->toBeTrue();
    expect(is_numeric($embedding[3]))->toBeTrue();
})->group('queenbee');
