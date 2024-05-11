<?php

use App\Livewire\PopularAgents;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(PopularAgents::class)
        ->assertStatus(200);
});
