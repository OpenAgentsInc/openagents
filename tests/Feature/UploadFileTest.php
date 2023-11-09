<?php

use App\Models\User;
use Illuminate\Http\UploadedFile;

test('upload a file via api', function () {
  $user = User::factory()->create();
  $this->actingAs($user);

  $file = UploadedFile::fake()->create('document.jsonl', 1000);

  $response = $this->postJson(route('files.store'), [
    'file' => $file
  ]);

  $response->assertRedirect();
  $response->assertSessionHas('message');
});
