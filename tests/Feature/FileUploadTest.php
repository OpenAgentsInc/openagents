<?php

use Illuminate\Http\UploadedFile;

test('unauthed user cannot upload a file', function () {
    $this->postJson('/api/files', [
        'file' => UploadedFile::fake()->image('avatar.jpg'),
    ])
        ->assertStatus(401);
});
