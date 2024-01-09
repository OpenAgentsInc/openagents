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
            ->component('AgentView')
        );
});

test('agent view knows its authors username', function () {
    $this->seed(ConciergeSeeder::class);

    $agent = Agent::first();

    // Anyone visiting agent/{id} sees the agent's nodes
    $response = $this->get('/agent/' . $agent->id);

    $response
        ->assertInertia(
            fn (Assert $page) => $page
            ->component('AgentView')
            ->has('owner')
            ->where('owner', $agent->user->username)
        );
});
