<?php

use App\Services\QueenbeeGateway;

test('can fetch github issue', function () {
  $response = GitHub::issues()->show('ArcadeLabsInc', 'openagents', 1);

  expect($response['url'])->toBe('https://api.github.com/repos/ArcadeLabsInc/openagents/issues/1');
  expect($response['repository_url'])->toBe('https://api.github.com/repos/ArcadeLabsInc/openagents');
  expect($response['body'])->toContain("We will implement the 'memory stream' architecture mentioned in the Generative Agents paper, excerpted below and slightly modified to reflect our 'autodev' use case.");
});

test('can respond to github issue as faerie', function () {
  $response = GitHub::issues()->show('ArcadeLabsInc', 'openagents', 1);

  $body = $response['body'];
  $title = $response['title'];

  $prompt = "You are Faerie, an AI agent specialized in writing & analyzing code.

You have been summoned to ArcadeLabsInc/openagents issue #1.
i
The issue is titled `" . $title . "`

The issue body is:
```
" . $body . "
```

Please respond with the comment you would like to add to the issue. Write like a senior developer would write; don't introduce yourself or use flowery text or a closing signature.";

  print_r($prompt);
  print_r("\n\n");

  $gateway = new QueenbeeGateway();

  $response = $gateway->makeChatCompletion([
    'model' => $gateway->defaultModel(),
    'messages' => [
      // ['role' => 'system', 'content' => 'You are a helpful assistant.'],
      ['role' => 'user', 'content' => $prompt],
    ],
  ]);

  print_r($response);

  $this->assertIsArray($response);
  $this->assertIsString($response['id']);
  $this->assertIsString($response['object']);
  $this->assertIsString($response['model']);
  $this->assertIsArray($response['choices']);
  $this->assertIsArray($response['usage']);
  $this->assertIsInt($response['created']);

});
