<?php

use App\Models\Agent;
use App\Models\User;
use Illuminate\Http\UploadedFile;
use Laravel\Sanctum\Sanctum;

use function Pest\Laravel\post;
use function Pest\Laravel\withoutExceptionHandling;

// Test adding a file to an agent
test('can add a file to an agent via api', function () {
    withoutExceptionHandling(); // Use this to get more detailed error messages if needed

    $user = User::factory()->create();
    // Ensure the agent is associated with the authenticated user
    $agent = Agent::factory()->create(['user_id' => $user->id]);

    Sanctum::actingAs($user);

    $file = UploadedFile::fake()->create('document.pdf', 1000, 'application/pdf'); // Create a fake file

    $response = post("/api/v1/agents/{$agent->id}/files", [
        'file' => $file,
        'description' => 'Test file description',
    ]);

    // Assuming $response holds your API call response
    $response->assertStatus(200);

    // Decode the response to an array to inspect it
    $responseData = $response->json();

    // Assert the structure and presence of specific keys/values
    $this->assertArrayHasKey('success', $responseData);
    $this->assertTrue($responseData['success']);
    $this->assertArrayHasKey('message', $responseData);
    $this->assertEquals('File added to agent successfully.', $responseData['message']);
    $this->assertArrayHasKey('data', $responseData);
    $this->assertArrayHasKey('file_id', $responseData['data']);
    $fileId = $responseData['data']['file_id'];

    // Now, assert in the database with dynamic file ID
    $this->assertDatabaseHas('files', [
        'id' => $fileId, // Using the file ID from the response
        'description' => 'Test file description',
        // Since 'path' is dynamically generated upon file storage, you may not predict the exact path,
        // but you can assert the presence of some part of the path you expect.
    ]);

});

// Test that a user can only upload a file to their own agent
test('user can only upload a file to their own agent', function () {
    $owner = User::factory()->create();
    $nonOwner = User::factory()->create();
    $agentOwnedByUser = Agent::factory()->for($owner)->create(); // Ensure the agent is owned by the 'owner' user

    Sanctum::actingAs($owner);

    $file = UploadedFile::fake()->create('owned_document.pdf', 1000, 'application/pdf'); // Create a fake file

    // Attempt by the owner to upload a file should succeed
    post("/api/v1/agents/{$agentOwnedByUser->id}/files", [
        'file' => $file,
        'description' => 'Owned test file description',
    ])
        ->assertStatus(200)
        ->assertJson([
            'success' => true,
            'message' => 'File added to agent successfully.',
            // Other assertions as necessary
        ]);

    Sanctum::actingAs($nonOwner);

    $fileForNonOwner = UploadedFile::fake()->create('non_owned_document.pdf', 1000, 'application/pdf'); // Create another fake file

    // Attempt by a non-owner to upload a file should fail
    post("/api/v1/agents/{$agentOwnedByUser->id}/files", [
        'file' => $fileForNonOwner,
        'description' => 'Non-owned test file description',
    ])
        ->assertStatus(403); // Assuming you return a 403 Forbidden status if the user doesn't own the agent
});

// Ensure an unauthenticated user cannot add a file to an agent
test('unauthenticated user cannot add a file to an agent', function () {
    $agent = Agent::factory()->create();

    $file = UploadedFile::fake()->create('unauthorized.pdf', 500, 'application/pdf');

    post("/api/v1/agents/{$agent->id}/files", [
        'file' => $file,
        'description' => 'Unauthorized file upload attempt',
    ], apiHeaders())
        ->assertStatus(401); // Expect a 401 Unauthorized response
});
