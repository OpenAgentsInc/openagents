<?php

use App\Models\Agent;
use App\Models\User;

test('user can see agent builder', function () {
    $user = User::factory()->create();
    $this->actingAs($user);

    Agent::factory()->create([
        'id' => 1,
        'user_id' => $user->id,
        'name' => 'Demo Agent',
        'description' => 'Agent Description',
    ]);


    $this->get('/agent/1/build')
        ->assertOk()
        ->assertSee('Demo Agent')
        ->assertSee('Agent Description')
        ->assertViewIs('agent-builder');
});
