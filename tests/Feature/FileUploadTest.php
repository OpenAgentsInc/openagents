<?php

use App\Jobs\IngestPDF;
use App\Models\File;
use App\Models\User;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;
use Illuminate\Support\Facades\Event;
use Illuminate\Support\Facades\Queue;

test('unauthed user cannot upload a file', function () {
    $this->postJson('/api/files', [
        'file' => UploadedFile::fake()->image('avatar.jpg'),
    ])
        ->assertStatus(401);
});

test('authed user can upload a file', function () {
    Queue::fake();
    Storage::fake('local');

    $user = User::factory()->create();
    $this->actingAs($user);

    $this->assertCount(0, File::all());

    $this->postJson('/api/files', [
        'file' => UploadedFile::fake()->image('avatar.pdf'),
    ])
        ->assertStatus(302);

    $this->assertCount(1, File::all());
    Queue::assertPushed(IngestPDF::class);
});
