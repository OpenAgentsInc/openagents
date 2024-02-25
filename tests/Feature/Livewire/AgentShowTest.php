<?php

use App\Models\Agent;
use App\Livewire\AgentShow;
use Livewire\Livewire;

it('renders successfully', function () {
    $agent = Agent::factory()->published()->create();
    Livewire::test(AgentShow::class, ['id' => $agent->id])
        ->assertStatus(200);
});
