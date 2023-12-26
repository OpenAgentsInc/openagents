<?php

use Database\Seeders\DatabaseSeeder;
use Inertia\Testing\AssertableInertia as Assert;

test('guest sees concierge nodes', function () {
    $this->seed(ConciergeSeeder::class);

    // Anyone visiting agent/{id} sees the agent's nodes
    $response = $this->get('/agent/1');

    $response->assertStatus(200)
        ->assertInertia(
            fn (Assert $page) => $page
            ->component('AgentNodes') // Replace with your actual component name
        );
});
