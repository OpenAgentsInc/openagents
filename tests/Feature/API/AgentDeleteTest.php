<?php

use App\Models\Agent;
use App\Models\User;
use Laravel\Sanctum\Sanctum;

use function Pest\Laravel\delete;

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

    $this->assertSoftDeleted('agents', [
        'id' => $agent->id,
    ]);
});

test('unauthenticated user cannot delete on agent', function () {
    $agent = Agent::factory()->create();

    // Attempt to delete an agent without authenticating
    delete("/api/v1/agents/{$agent->id}", [], apiHeaders())
        ->assertStatus(401); // Expect a 401 Unauthorized response
});
