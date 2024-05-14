<?php

use App\Livewire\AgentProfile;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(AgentProfile::class)
        ->assertStatus(200);
});
