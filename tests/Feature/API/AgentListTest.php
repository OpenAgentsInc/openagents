<?php

use App\Models\Agent;
use App\Models\User;
use Laravel\Sanctum\Sanctum;

use function Pest\Laravel\get;

// Test that an authenticated user can retrieve a list of agents
test('authenticated user can retrieve list of agents', function () {
    $user = User::factory()->create();
    // Create several agents for this user
    Agent::factory()->count(5)->create(['user_id' => $user->id]);

    Sanctum::actingAs($user);

    get('/api/v1/agents')
        ->assertStatus(200)
        ->assertJsonCount(5, 'data') // Assuming the agents are wrapped in a 'data' key
        ->assertJsonStructure([
            'data' => [
                '*' => [
                    'id',
                    'name',
                    'description',
                    'instructions',
                    'welcome_message',
                    'user_id',
                ],
            ],
        ]);
});

// Test that an unauthenticated user cannot retrieve the list of agents
test('unauthenticated user cannot retrieve list of agents', function () {
    get('/api/v1/agents', apiHeaders())
        ->assertStatus(401);
});
