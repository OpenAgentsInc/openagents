<?php

use App\Livewire\Navbar;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Navbar::class)
        ->assertStatus(200);
});

it('shows at the home route /', function () {
    $this->get('/')
        ->assertStatus(200)
        ->assertSeeLivewire('navbar');
});
