<?php

use App\Models\Agent;
use App\Models\User;
use Illuminate\Http\UploadedFile;
use Laravel\Sanctum\Sanctum;

use function Pest\Laravel\delete;
use function Pest\Laravel\post;
use function Pest\Laravel\withoutExceptionHandling;

// Helper method for storing a file for testing purposes
function storeFile(Agent $agent, UploadedFile $file): array
{
    $response = post("/api/v1/agents/{$agent->id}/files", [
        'file' => $file,
        'description' => 'Test file description',
    ]);

    return $response->json();
}

// Helper function for deleting a file for testing purposes
function deleteFile(File $file): void
{
    $response = delete("/api/v1/files/{$file->id}");

    // Assert the response
    $response->assertStatus(204);
}

test('can update a file via api', function () {
    withoutExceptionHandling();

    $user = User::factory()->create();
    $agent = Agent::factory()->create(['user_id' => $user->id]);
    $file = UploadedFile::fake()->create('document.pdf', 1000, 'application/pdf');

    Sanctum::actingAs($user);

    $updatedFile = storeFile($agent, $file);

    dd($updatedFile);

    $updatedData = [
        'name' => 'Updated file name',
        'description' => 'Updated file description',
        'path' => $updatedFile['path'],
        'agent_id' => $updatedFile['agent_id'],
    ];

    $response = $this->updateFile($updatedFile['id'], $updatedData);

    $response->assertStatus(200);

    $responseData = $response->json();

    $this->assertArrayHasKey('success', $responseData);
    $this->assertTrue($responseData['success']);
    $this->assertArrayHasKey('message', $responseData);
    $this->assertEquals('File updated successfully.', $responseData['message']);
    $this->assertArrayHasKey('data', $responseData);
    $updatedFileId = $updatedFile['id'];

    $this->assertDatabaseHas('files', [
        'id' => $updatedFileId,
        'name' => $updatedData['name'],
        'description' => $updatedData['description'],
    ]);
})->skip();

test('can delete a file via api', function () {
    // withoutExceptionHandling(); // Uncomment if you want to see exceptions

    $user = User::factory()->create();
    $agent = Agent::factory()->create(['user_id' => $user->id]);

    Sanctum::actingAs($user);

    $file = UploadedFile::fake()->create('document.pdf', 1000, 'application/pdf');
    $response = $this->postJson('/api/v1/agents/'.$agent->id.'/files', [
        'file' => $file,
        'description' => 'Test file description',
    ]);

    $createdFile = $response->json('data');

    // Now let's delete the file
    $response = $this->deleteJson('/api/v1/files/'.$createdFile['file_id']);

    $response->assertStatus(200);

    $responseData = $response->json();

    $this->assertArrayHasKey('success', $responseData);
    $this->assertTrue($responseData['success']);
    $this->assertArrayHasKey('message', $responseData);
    $this->assertEquals('File deleted successfully.', $responseData['message']);

    $this->assertDatabaseMissing('files', [
        'id' => $createdFile['file_id'],
    ]);
});
