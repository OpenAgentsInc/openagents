<?php

use App\Livewire\AgentShow;
use App\Models\Agent;
use Livewire\Livewire;

it('renders successfully', function () {
    $agent = Agent::factory()->published()->create();
    Livewire::test(AgentShow::class, ['id' => $agent->id])
        ->assertStatus(200);
});
