<?php

use App\Models\File;

test('anyone can query their uploaded file via api', function () {

  // Given there is an uploaded file
  $file = File::factory()->withEmbeddings(5)->create();

  $response = $this->postJson(route('query.store'), [
    'file_id' => $file->id,
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
