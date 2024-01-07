<?php

use App\Models\Agent;
use App\Models\User;

$properPayload = [
    'name' => 'John Doe',
    'description' => 'This is a description',
];

test('name is required to create an agent', function () use ($properPayload) {
    $user = User::factory()->create();
    $this->actingAs($user);

    $this->assertCount(0, Agent::all());

    $this->postJson('/api/agents', [
        ...$properPayload,
        'name' => '',
    ])
      ->assertStatus(422)
      ->assertJsonValidationErrors('name');

    // expect that there are 0 agents
    $this->assertCount(0, Agent::all());
});

test('description is required to create an agent', function () use ($properPayload) {
    $user = User::factory()->create();
    $this->actingAs($user);

    $this->assertCount(0, Agent::all());

    $this->postJson('/api/agents', [
        ...$properPayload,
        'description' => '',
    ])
      ->assertStatus(422)
      ->assertJsonValidationErrors('description');

    // expect that there are 0 agents
    $this->assertCount(0, Agent::all());
});
