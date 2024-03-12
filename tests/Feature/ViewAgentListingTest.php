<?php

use App\Models\Agent;

test('user can view a published agent listing', function () {
    $agent = Agent::factory()->published()->create([
        'name' => 'Test Agent',
        'description' => 'This is a test agent',
    ]);

    $this->get('/agent/'.$agent->id)
        ->assertStatus(200)
        ->assertSee('Test Agent')
        ->assertSee('This is a test agent');
});

test('user cannot view an unpublished agent listing', function () {
    $agent = Agent::factory()->unpublished()->create([
        'name' => 'Test Agent',
        'description' => 'This is a test agent',
    ]);

    $this->get('/agent/'.$agent->id)
        ->assertStatus(404);
});
