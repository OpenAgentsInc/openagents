<?php

use App\Livewire\AgentCard;
use App\Models\Agent;
use Livewire\Livewire;

it('renders successfully', function () {
    $agent = Agent::factory()->create();
    Livewire::test(AgentCard::class, ['agent' => $agent])
        ->assertStatus(200);
});
