<?php

use App\Livewire\Login;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Login::class)
        ->assertStatus(200);
});

it('shows at the login route /login', function () {
    $this->get('/login')
        ->assertStatus(200)
        ->assertSeeLivewire('login');
});
