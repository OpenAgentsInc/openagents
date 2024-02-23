<?php

use App\Livewire\AgentShow;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(AgentShow::class)
        ->assertStatus(200);
});
