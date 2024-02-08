<?php

use App\Models\Agent;
use App\Models\User;

test('user can see create agent form', function () {
    $user = User::factory()->create();
    $this->actingAs($user);

    $this->get('/agents/create')
        ->assertOk()
        ->assertSee('Create Agent')
        ->assertViewIs('agent-create');
});

test('unauthed user cannot create agent', function () {
    $this->post('/agents', [
        'name' => 'Count Vowels',
        'description' => 'Count the vowels in a string',
        'instructions' => 'Count the vowels in a string',
        'welcome_message' => 'Welcome to the Count Vowels agent!',
    ])->assertRedirect('/login');
});

test('user can create agent', function () {
    $user = User::factory()->create();
    $this->actingAs($user);

    expect(Agent::all())->toHaveCount(0);

    $this->post('/agents', [
        'name' => 'Count Vowels',
        'description' => 'Count the vowels in a string',
        'instructions' => 'Count the vowels in a string',
        'welcome_message' => 'Welcome to the Count Vowels agent!',
    ]);

    expect(Agent::all())->toHaveCount(1);
});
