<?php

use App\Livewire\Navbar;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Navbar::class)
        ->assertStatus(200);
});
