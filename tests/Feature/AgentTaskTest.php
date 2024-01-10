<?php

use App\Models\Agent;
use App\Models\Step;
use App\Models\Task;
use App\Models\User;

test('new agents are created with default task and step', function () {

    // Given we have a user
    $user = User::factory()->create();
    $this->actingAs($user);

    // And we have no agents or tasks/steps
    $this->assertCount(0, Agent::all());
    $this->assertCount(0, Task::all());
    $this->assertCount(0, Step::all());

    // When we create an agent
    $this->postJson(route('agents.store'), [
        'name' => 'John Doe',
        'description' => 'This is a description',
        'instructions' => 'This is a set of instructions',
        'welcome_message' => 'This is a welcome message'
    ])
        ->assertStatus(201);

    // Then we have 1 agent
    $this->assertCount(1, Agent::all());
    $this->assertCount(1, Task::all());
    $this->assertCount(2, Step::all());
});
