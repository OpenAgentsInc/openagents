<?php

use App\Models\File;

it('has many embeddings', function () {
    $file = File::factory()->create();
    $file->embeddings()->createMany([
        ['embedding' => array_fill(0, 768, 0), 'metadata' => [
          'text' => 'A lot of zeros',
        ]],
        ['embedding' => array_fill(0, 768, 0.5), 'metadata' => [
          'text' => 'A lot of halves',
        ]],
    ]);

    expect($file->embeddings)->toHaveCount(2);
});
