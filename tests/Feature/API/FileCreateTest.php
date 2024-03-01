<?php

use App\Models\Agent;
use App\Models\User;
use Illuminate\Http\UploadedFile;
use Laravel\Sanctum\Sanctum;

test('can create a file via api', function () {
    $user = User::factory()->create();
    $agent = Agent::factory()->create(['user_id' => $user->id]);
    $file = UploadedFile::fake()->create('document.pdf', 1000, 'application/pdf');

    Sanctum::actingAs($user);

    // Prepare file data
    $data = [
        'description' => 'Test file description',
        'path' => '/path/to/file',
        'agent_id' => $agent->id,
    ];

    // Create the file
    $response = $this->postJson('/api/v1/files', $data);

    $response->assertStatus(201);

    $responseData = $response->json();

    $this->assertArrayHasKey('success', $responseData);
    $this->assertTrue($responseData['success']);
    $this->assertArrayHasKey('message', $responseData);
    $this->assertEquals('File created successfully.', $responseData['message']);
    $this->assertArrayHasKey('data', $responseData);
    $this->assertArrayHasKey('file_id', $responseData['data']);

    // Check that the file was created in the database
    $this->assertDatabaseHas('files', [
        'user_id' => $user->id,
        'description' => $data['description'],
        'path' => $data['path'],
    ]);
});
