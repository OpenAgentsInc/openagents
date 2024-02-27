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
    $agent = Agent::factory()->create();

    Sanctum::actingAs($user);

    $file = UploadedFile::fake()->create('document.pdf', 1000, 'application/pdf'); // Create a fake file

    post("/api/v1/agents/{$agent->id}/files", [
        'file' => $file,
        'description' => 'Test file description',
    ])
        ->assertStatus(200)
        ->assertJson([
            'success' => true,
            'message' => 'File added to agent successfully.',
            'data' => [
                'agent_id' => $agent->id,
                // 'file_id' => exists, // You can't predict the file ID here, but you can check if it's returned
            ],
        ]);

    // Optionally, assert the file was indeed added in the database
    // $this->assertDatabaseHas('agent_files', [
    //     'agent_id' => $agent->id,
    //     'description' => 'Test file description',
    //     // Make sure to include any other relevant fields
    // ]);
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
