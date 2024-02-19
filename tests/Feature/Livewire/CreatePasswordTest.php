<?php

use App\Livewire\CreatePassword;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(CreatePassword::class)
        ->assertStatus(200);
});

it('shows at the create password route /create-password', function () {
    $this->get('/create-password')
        ->assertStatus(200)
        ->assertSeeLivewire('create-password');
});
