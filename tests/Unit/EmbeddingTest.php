<?php

use App\Models\Embedding;
use App\Models\File;

it('belongs to a file', function () {
  $file = File::factory()->create();

  // Create the embedding instance without saving it to the database
  $embedding = Embedding::factory()->make();

  // Associate the file and then save
  $embedding->file()->associate($file);

  $embedding->save();

  expect($embedding->file)->toBeInstanceOf(File::class);
});
