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

it('shows login form', function () {
    $this->get('/login')
        ->assertStatus(200)
        ->assertSee('Email');
});

it('can be submitted', function () {
    Livewire::test(Login::class)
        ->set('email', 'blam@blam.com')
        ->call('submit')
        ->assertHasNoErrors();
});

it('redirects to create password if user does not exist', function () {
    Livewire::test(Login::class)
        ->set('email', 'blam@blam.com')
        ->call('submit')
        ->assertHasNoErrors()
        ->assertRedirect('/create-password')
        ->assertSessionHas('email_for_password_creation', 'blam@blam.com');
});
