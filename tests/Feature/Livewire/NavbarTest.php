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

it('shows a chat button', function () {
    $this->get('/')
        ->assertStatus(200)
        ->assertSee('Chat');
});

it('shows login & register buttons if unauthed', function () {
    $this->get('/')
        ->assertStatus(200)
        ->assertSee('Login')
        ->assertSee('Register');
});
