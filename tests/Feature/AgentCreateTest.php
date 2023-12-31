<?php

use App\Models\Agent;
use App\Models\User;

$properPayload = [
    'name' => 'John Doe',
    'description' => 'This is a description',
    'instructions' => 'This is a set of instructions',
    'welcome_message' => 'This is a welcome message'
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

test('instructions is required to create an agent', function () use ($properPayload) {
    $user = User::factory()->create();
    $this->actingAs($user);

    $this->assertCount(0, Agent::all());

    $this->postJson('/api/agents', [
        ...$properPayload,
        'instructions' => '',
    ])
      ->assertStatus(422)
      ->assertJsonValidationErrors('instructions');

    // expect that there are 0 agents
    $this->assertCount(0, Agent::all());
});

test('welcome message is required to create an agent', function () use ($properPayload) {
    $user = User::factory()->create();
    $this->actingAs($user);

    $this->assertCount(0, Agent::all());

    $this->postJson('/api/agents', [
        ...$properPayload,
        'welcome_message' => '',
    ])
      ->assertStatus(422)
      ->assertJsonValidationErrors('welcome_message');

    // expect that there are 0 agents
    $this->assertCount(0, Agent::all());
});
