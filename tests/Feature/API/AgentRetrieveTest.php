<?php

use App\Models\Agent;
use App\Models\User;
use Laravel\Sanctum\Sanctum;

use function Pest\Laravel\get;

test('can retrieve an agent via api', function () {
    $user = User::factory()->create();
    $agent = Agent::factory()->create();

    Sanctum::actingAs($user);

    get("/api/v1/agents/{$agent->id}")
        ->assertStatus(200)
        ->assertJson([
            'success' => true,
            'data' => [
                'id' => $agent->id,
                'name' => $agent->name,
            ],
        ]);
});

test('unauthenticated user cannot retrieve agent', function () {
    $agent = Agent::factory()->create();

    // Attempt to retrieve an agent without authenticating
    get("/api/v1/agents/{$agent->id}", apiHeaders())
        ->assertStatus(401); // Expect a 401 Unauthorized response
});
