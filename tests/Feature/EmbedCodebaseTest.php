<?php

use App\Models\File;

test('can create embeddings from a file', function () {
  // Given a file (for now let's hardcode app/models/Agent.php)
  $file = File::factory()->create([
    'path' => 'app/models/Agent.php',
  ]);

  // Create embeddings for the file
  $file->createEmbeddings();

  // Assert that there are embeddings
  expect($file->embeddings()->count())->toBeGreaterThan(0);
});


// create embeddings from a folder
// create embeddings from entire codebase
