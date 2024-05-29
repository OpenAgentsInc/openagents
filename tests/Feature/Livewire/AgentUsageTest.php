<?php

use App\Livewire\AgentUsage;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(AgentUsage::class)
        ->assertStatus(200);
})->skip();
