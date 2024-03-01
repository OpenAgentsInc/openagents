<?php

use App\Models\Agent;
use App\Models\User;
use Laravel\Sanctum\Sanctum;

use function Pest\Laravel\post;

test('can create thread via api', function () {
    $user = User::factory()->create();
    $agent = Agent::factory()->create();

    $this->assertDatabaseCount('threads', 0);

    Sanctum::actingAs($user);

    post('/api/v1/threads', [
        'agent_id' => $agent->id,
    ])
        ->assertStatus(200)
        ->assertJson([
            'success' => true,
            'data' => [],
        ])
        ->assertJsonStructure([
            'success',
            'data' => ['agent_id', 'id'],
        ]);

    $this->assertDatabaseCount('threads', 1);
});
