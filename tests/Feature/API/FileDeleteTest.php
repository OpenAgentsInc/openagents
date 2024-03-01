<?php

use App\Models\Agent;
use App\Models\User;
use Illuminate\Http\UploadedFile;
use Laravel\Sanctum\Sanctum;

test('can delete a file via api', function () {
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
})->skip();
