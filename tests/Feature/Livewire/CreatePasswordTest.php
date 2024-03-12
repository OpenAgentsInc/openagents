<?php

use App\Livewire\CreatePassword;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(CreatePassword::class)
        ->assertStatus(302);
});

it('redirects to login', function () {
    $this->get('/create-password')
        ->assertRedirect('login');
});
