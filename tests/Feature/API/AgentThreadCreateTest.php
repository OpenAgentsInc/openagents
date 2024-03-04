<?php

/**
 * Tests to ensure that agents and threads can be associated.
 */

use App\Models\Agent;
use App\Models\Thread;
use App\Models\User;
use Laravel\Sanctum\Sanctum;

use function Pest\Laravel\assertDatabaseHas;
use function Pest\Laravel\post;

test('can create an association between an agent and a thread via api', function () {
    $user = User::factory()->create();
    Sanctum::actingAs($user);

    $agent = Agent::factory()->create();
    $thread = Thread::factory()->create();

    $threadData = [
        // Assuming the API requires a title for the thread creation
        'thread_id' => $thread->id,
    ];

    // Using the store endpoint to create a new thread and associate it with the agent
    post("/api/v1/agents/{$agent->id}/threads", $threadData)
        ->assertStatus(200)
        ->assertJson([
            'success' => true,
            'message' => 'Agent associated with thread successfully',
        ]);

    // Assert that the relationship was indeed created in the intermediary table
    // Replace 'agent_thread' with the actual name of your pivot table
    // and 'agent_id', 'thread_id' with the actual field names
    assertDatabaseHas('agent_thread', [
        'agent_id' => $agent->id,
        'thread_id' => $thread->id, // Use the retrieved ID of the created thread
    ]);
});
