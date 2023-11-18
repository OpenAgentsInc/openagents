<?php

use App\Models\File;
use App\Models\Embedding;
use App\Services\Embedder;

test('can create embeddings from a file', function () {
  // Given a file (for now let's hardcode app/models/Agent.php)
  $file = File::factory()->create([
    'path' => 'app/models/Agent.php',
  ]);

  // Create embeddings for the file
  $file->createEmbeddings();

  // Assert that there are embeddings
  expect($file->embeddings()->count())->toBeGreaterThan(0);
})->group('integration');


// create embeddings from a folder
test('can create embeddings from a folder', function () {
  // Given a folder (for now let's hardcode app/models)
  $folder = 'app/Models';

  // Create embeddings for the folder
  $embedder = new Embedder();
  $embedder->createEmbeddingsForFolder($folder);

  // Assert that there are embeddings
  expect(Embedding::query()->count())->toBeGreaterThan(0);

  // Assert that number of Files is equal to number of Embeddings
  expect(File::query()->count())->toBe(Embedding::query()->count());
})->group('integration');


// create embeddings from entire codebase
