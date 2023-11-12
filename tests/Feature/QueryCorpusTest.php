<?php

test('anyone can query corpus via api', function () {
  $response = $this->postJson(route('query.store'), [
    'corpus_id' => 4,
    'query' => "What is a softbot?",
  ]);

  $response->assertStatus(200);
  $response->assertJson([
    'ok' => true,
  ]);

  $response->assertJsonStructure([
    'results' => [
      '*' => [
        'text',
        'score',
        'metadata',
      ],
    ],
  ]);
});
