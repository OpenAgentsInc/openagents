<?php

use App\Services\QueenbeeGateway;
use App\Services\Searcher;

test('can summarize from context', function () {
  $data = [
    ['text' => 'The meaning of life is 42.'],
    ['text' => 'That saying comes from the Hitchhiker\'s Guide to the Galaxy.'],
    ['text' => 'The book is a comedy science fiction series created by Douglas Adams.'],
  ];
  $searcher = new Searcher();
  $summary = $searcher->summarize($data, 'What is the meaning of life?');

  $this->assertIsString($summary);
  $this->assertGreaterThanOrEqual(50, strlen($summary));
});


test('can do chat completion', function () {
  $gateway = new QueenbeeGateway();

  $response = $gateway->makeChatCompletion([
    'model' => $gateway->defaultModel(),
    'messages' => [
      ['role' => 'system', 'content' => 'You are a helpful assistant.'],
      ['role' => 'user', 'content' => 'Hello!'],
    ],
  ]);

  $this->assertIsArray($response);
  $this->assertIsString($response['id']);
  $this->assertIsString($response['object']);
  $this->assertIsString($response['model']);
  $this->assertIsArray($response['choices']);
  $this->assertIsArray($response['usage']);
  $this->assertIsInt($response['created']);
});
