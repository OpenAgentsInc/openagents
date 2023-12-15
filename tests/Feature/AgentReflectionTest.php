<?php

use App\Models\Agent;
use App\Models\Step;

test('agent can reflect', function () {
    // Given an agent
    $agent = Agent::factory()->create();
    $step = Step::factory()->create(['agent_id' => $agent->id]);

    // The agent can reflect
    $agent->reflect();

    // This creates one or more thoughts
    $this->assertDatabaseCount('thoughts', 1);
    $this->assertDatabaseHas('thoughts', [
        'agent_id' => $agent->id,
    ]);
})->group('integration');
