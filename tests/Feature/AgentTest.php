<?php

use App\Models\Agent;
use App\Models\User;

test('user can create an agent via api', function () {
  $user = User::factory()->create();
  $this->actingAs($user);

  $this->assertCount(0, Agent::all());

  $this->postJson('/api/agents', [
    'name' => 'Jane Doe',
  ])->assertStatus(201);

  $this->assertCount(1, Agent::all());
  $this->assertEquals('Jane Doe', Agent::first()->name);
  $this->assertEquals($user->id, Agent::first()->user_id);
});

test('unauthenticated user cannot create an agent via api', function () {
  $this->assertCount(0, Agent::all());

  $this->postJson('/api/agents', [
    'name' => 'John Doe',
  ])
    ->assertUnauthorized()
    ->assertStatus(401);

  // expect that there are 0 agents
  $this->assertCount(0, Agent::all());
});

test('name is required to create an agent', function () {
  $user = User::factory()->create();
  $this->actingAs($user);

  $this->assertCount(0, Agent::all());

  $this->postJson('/api/agents', [
    'name' => '',
  ])
    ->assertStatus(422)
    ->assertJsonValidationErrors('name');

  // expect that there are 0 agents
  $this->assertCount(0, Agent::all());
});

// user can update an agent
// user can delete an agent
