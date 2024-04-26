<?php

use App\Livewire\CreateAgent;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(CreateAgent::class)
        ->assertStatus(200);
});
