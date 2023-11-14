<?php

use App\Services\QueenbeeGateway;

test('can create embedding from text', function () {

  $gateway = new QueenbeeGateway();
  $embedding = $gateway->createEmbedding("What is an AI agent?");

  // Check if the array contains exactly 768 elements
  expect(count($embedding))->toBe(768);

  // Optionally, check if all elements are numeric or within a certain range
  foreach ($embedding as $value) {
    expect(is_numeric($value))->toBeTrue();
  }

});
