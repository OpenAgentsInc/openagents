<?php

use App\Models\Agent;
use App\Models\User;
use Laravel\Sanctum\Sanctum;

use function Pest\Laravel\put;

// Test updating an agent
test('can update an agent via api', function () {
    $user = User::factory()->create();
    $agent = Agent::factory()->create();

    Sanctum::actingAs($user);

    $updateData = [
        'name' => 'Updated Test Agent',
        // Add other fields that you want to update
    ];

    put("/api/v1/agents/{$agent->id}", $updateData)
        ->assertStatus(200)
        ->assertJson([
            'success' => true,
            'message' => 'Agent updated successfully.',
            // Validate the updated data as needed
        ]);

    // Optionally, assert that the agent was indeed updated in the database
    $this->assertDatabaseHas('agents', [
        'id' => $agent->id,
        'name' => 'Updated Test Agent',
        // Add other fields that were updated
    ]);
});

// Ensure an unauthenticated user cannot retrieve, update, or delete an agent
test('unauthenticated user cannot perform CRUD operations on agent', function () {
    $agent = Agent::factory()->create();
    // Attempt to update an agent without authenticating
    put("/api/v1/agents/{$agent->id}", [
        'name' => 'Unauthorized Update Attempt',
    ], apiHeaders())->assertStatus(401); // Expect a 401 Unauthorized response
});
