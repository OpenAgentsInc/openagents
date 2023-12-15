<?php

test('agent can reflect', function () {
    // Given an agent
    $agent = Agent::factory()->create();

    // The agent can reflect
    $this->assertTrue($agent->can('reflect'));
    $agent->reflect();

    // This creates one or more thoughts
    $this->assertDatabaseCount('thoughts', 1);
    $this->assertDatabaseHas('thoughts', [
        'agent_id' => $agent->id,
    ]);
});
