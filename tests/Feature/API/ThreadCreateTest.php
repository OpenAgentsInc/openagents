<?php

use App\Models\User;
use Laravel\Sanctum\Sanctum;

use function Pest\Laravel\post;

test('can create thread via api', function () {
    $user = User::factory()->create();

    Sanctum::actingAs($user);

    post('/api/v1/threads', [
        'agent_id' => 1,
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

    // Optionally, you can also assert that the agent was indeed created in the database
    //    $this->assertDatabaseHas('agents', [
    //        'name' => 'Test Agent',
    //        'description' => 'This is a test agent',
    //        'instructions' => 'This is a test instruction',
    //    ]);
});
