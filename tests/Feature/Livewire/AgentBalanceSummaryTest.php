<?php

use App\Livewire\AgentBalanceSummary;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(AgentBalanceSummary::class)
        ->assertStatus(200);
});
