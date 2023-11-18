<?php

use App\Services\QueenbeeGateway;

test('can fetch github issue', function () {

  $response = GitHub::issues()->show('ArcadeLabsInc', 'openagents', 1);

  expect($response['url'])->toBe('https://api.github.com/repos/ArcadeLabsInc/openagents/issues/1');
  expect($response['repository_url'])->toBe('https://api.github.com/repos/ArcadeLabsInc/openagents');
  expect($response['body'])->toContain("We will implement the 'memory stream' architecture mentioned in the Generative Agents paper, excerpted below and slightly modified to reflect our 'autodev' use case.");

});
