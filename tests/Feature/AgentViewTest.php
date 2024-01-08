<?php

use App\Models\Agent;
use Database\Seeders\DatabaseSeeder;
use Inertia\Testing\AssertableInertia as Assert;

test('guest sees view agent page', function () {
    $this->seed(ConciergeSeeder::class);

    $agentId = Agent::first()->id;

    // Anyone visiting agent/{id} sees the agent's nodes
    $response = $this->get('/agent/' . $agentId);

    $response // ->assertStatus(200)
        ->assertInertia(
            fn (Assert $page) => $page
            ->component('AgentView') // Replace with your actual component name
        );
});
