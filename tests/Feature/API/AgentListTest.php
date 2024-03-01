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
<<<<<<< HEAD
        ->assertJsonCount(5) // Directly assert the count of agents in the root of the JSON response
        ->assertJsonStructure([
            '*' => [
                'id',
                'name',
                'description',
                'instructions',
                'welcome_message',
                'user_id',
=======
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
>>>>>>> 76f4603b487876635961671e7ac4af1745e7ad8e
            ],
        ])
        ->assertJsonPath('success', true)
        ->assertJsonCount(5, 'data');
});

test('unauthenticated user cannot retrieve list of agents', function () {
    get('/api/v1/agents', apiHeaders())
        ->assertStatus(401);
});
