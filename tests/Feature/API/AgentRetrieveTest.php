<?php

use App\Models\Agent;
use App\Models\User;
use Laravel\Sanctum\Sanctum;

use function Pest\Laravel\delete;
use function Pest\Laravel\get;
use function Pest\Laravel\put;

// Test retrieving an agent
test('can retrieve an agent via api', function () {
    $user = User::factory()->create();
    $agent = Agent::factory()->create(); // Assuming you have a factory for Agents

    Sanctum::actingAs($user);

    get("/api/v1/agents/{$agent->id}")
        ->assertStatus(200)
        ->assertJson([
            'success' => true,
            'data' => [
                'id' => $agent->id,
                'name' => $agent->name,
                // Add other fields as necessary
            ],
        ]);
});

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

// Test deleting an agent
test('can delete an agent via api', function () {
    $user = User::factory()->create();
    $agent = Agent::factory()->create();

    Sanctum::actingAs($user);

    delete("/api/v1/agents/{$agent->id}")
        ->assertStatus(200)
        ->assertJson([
            'success' => true,
            'message' => 'Agent deleted successfully',
        ]);

    // Optionally, you can assert that the agent no longer exists in the database
    $this->assertSoftDeleted('agents', [
        'id' => $agent->id,
    ]);
});

// Ensure an unauthenticated user cannot retrieve, update, or delete an agent
test('unauthenticated user cannot perform CRUD operations on agent', function () {
    $agent = Agent::factory()->create();

    // Attempt to retrieve an agent without authenticating
    $this->withHeaders(['Accept' => 'application/json'])
        ->get("/api/v1/agents/{$agent->id}")
        ->assertStatus(401); // Expect a 401 Unauthorized response

    // Attempt to update an agent without authenticating
    $this->withHeaders(['Accept' => 'application/json'])
        ->put("/api/v1/agents/{$agent->id}", [
            'name' => 'Unauthorized Update Attempt',
        ])->assertStatus(401); // Expect a 401 Unauthorized response

    // Attempt to delete an agent without authenticating
    $this->withHeaders(['Accept' => 'application/json'])
        ->delete("/api/v1/agents/{$agent->id}")
        ->assertStatus(401); // Expect a 401 Unauthorized response
});
