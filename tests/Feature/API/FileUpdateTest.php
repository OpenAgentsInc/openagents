<?php

use App\Models\Agent;
use App\Models\User;
use Illuminate\Http\UploadedFile;
use Laravel\Sanctum\Sanctum;

use function Pest\Laravel\withoutExceptionHandling;

test('can update a file via api', function () {
    withoutExceptionHandling(); // Uncomment if you encounter an exception

    $user = User::factory()->create();
    $agent = Agent::factory()->create(['user_id' => $user->id]);
    $file = UploadedFile::fake()->create('document.pdf', 1000, 'application/pdf');

    Sanctum::actingAs($user);

    // Store a file first
    $createdFileResponse = $this->postJson("/api/v1/agents/{$agent->id}/files", [
        'file' => $file,
        'description' => 'Test file description',
    ]);

    $createdFileData = $createdFileResponse->json('data');

    // Prepare updated file data
    $updatedData = [
        'description' => 'Updated file description',
    ];

    // Update the file
    $response = $this->putJson("/api/v1/files/{$createdFileData['file_id']}", $updatedData);

    $response->assertStatus(200);

    $responseData = $response->json();

    $this->assertArrayHasKey('success', $responseData);
    $this->assertTrue($responseData['success']);
    $this->assertArrayHasKey('message', $responseData);
    $this->assertEquals('File updated successfully.', $responseData['message']);
    $this->assertArrayHasKey('data', $responseData);

    // Check that the file was updated in the database
    $this->assertDatabaseHas('files', [
        'id' => $createdFileData['file_id'],
        'description' => $updatedData['description'],
    ]);
});
