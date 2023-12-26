<?php

use App\Models\Agent;
use Database\Seeders\ConciergeSeeder;

test('chat message sent to an agent executes its task', function () {
    // Given we have a Concierge agent
    $this->seed(ConciergeSeeder::class);
    $agent = Agent::findOrFail(1);
    $this->expect($agent->name)->toBe('The Concierge');

    // And I as guest message the agent
    $response = $this->post('/agent/1/chat', ['body' => 'Hello, world!']);

    // The response is successful
    $response->assertStatus(200);

    // It creates a message

    // And creates four steps

});
