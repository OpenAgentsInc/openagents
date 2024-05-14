<?php

use App\Livewire\AgentCard;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(AgentCard::class)
        ->assertStatus(200);
});
