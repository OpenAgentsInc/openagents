<?php

use App\Livewire\AgentShow;
use Livewire\Livewire;

it('renders successfully', function () {
    $agent = \App\Models\Agent::factory()->create();
    Livewire::test(AgentShow::class, ['id' => $agent->id])
        ->assertStatus(200);
});
