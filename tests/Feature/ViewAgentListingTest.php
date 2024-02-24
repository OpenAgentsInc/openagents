<?php

use App\Models\Agent;

test('user can view agent listing', function () {
    $agent = Agent::factory()->create([
        'name' => 'Test Agent',
        'description' => 'This is a test agent',
    ]);

    $this->get('/agent/' . $agent->id)
        ->assertStatus(200)
        ->assertSee('Test Agent')
        ->assertSee('This is a test agent');
});
