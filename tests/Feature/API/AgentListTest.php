<?php

use App\Models\Agent;
use App\Models\User;
use Laravel\Sanctum\Sanctum;

use function Pest\Laravel\get;

test('authenticated user can retrieve list of agents', function () {
    $user = User::factory()->create();
    // Create several agents for this user
    Agent::factory()->count(5)->create(['user_id' => $user->id]);

    Sanctum::actingAs($user);

    get('/api/v1/agents')
        ->assertStatus(200)
        ->assertJsonStructure([
            'success',
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
        ])
        ->assertJsonPath('success', true)
        ->assertJsonCount(5, 'data');
});

test('unauthenticated user cannot retrieve list of agents', function () {
    get('/api/v1/agents', apiHeaders())
        ->assertStatus(401);
});
