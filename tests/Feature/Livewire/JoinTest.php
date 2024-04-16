<?php

use App\Livewire\Auth\Join;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Join::class)
        ->assertStatus(200);
});
