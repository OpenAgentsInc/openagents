<?php

use App\Livewire\AgentBalanceSummary;
use App\Models\User;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::actingAs(User::factory()->create())
        ->test(AgentBalanceSummary::class)
        ->assertStatus(200);
});
