<?php

use App\Models\File;
use App\Models\User;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;

test('unauthed user cannot upload a file', function () {
    $this->postJson('/api/files', [
        'file' => UploadedFile::fake()->image('avatar.jpg'),
    ])
        ->assertStatus(401);
});

test('authed user can upload a file', function () {
    // fake storage
    Storage::fake('local');

    $user = User::factory()->create();
    $this->actingAs($user);

    $this->assertCount(0, File::all());

    $this->postJson('/api/files', [
        'file' => UploadedFile::fake()->image('avatar.pdf'),
    ])
        ->assertStatus(302);

    // expect that there is 1 file
    // $this->assertCount(1, File::all());

    $this->assertCount(1, File::all());
});
