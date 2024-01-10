<?php

use App\Models\Agent;
use App\Models\User;

test('new agents are created with default task and step', function () {

    // Given we have a user
    $user = User::factory()->create();
    $this->actingAs($user);

    // And we have no agents
    $this->assertCount(0, Agent::all());

    // When we create an agent
    $this->postJson('/api/agents', [
        'name' => 'John Doe',
        'description' => 'This is a description',
        'instructions' => 'This is a set of instructions',
        'welcome_message' => 'This is a welcome message'
    ])
        ->assertStatus(201)
        ->assertJsonStructure(['id', 'name', 'description', 'instructions', 'welcome_message', 'created_at', 'updated_at']);

    // Then we have 1 agent
    $this->assertCount(1, Agent::all());

    // And the agent has 1 task
    $agent = Agent::first();
    $this->assertCount(1, $agent->tasks);

    // And the task has 1 step
    $task = $agent->tasks->first();
    $this->assertCount(1, $task->steps);

    // And the step has 1 message
    $step = $task->steps->first();
    $this->assertCount(1, $step->messages);

    // And the message is the welcome message
    $message = $step->messages->first();
    $this->assertEquals('This is a welcome message', $message->body);

})->skip();
