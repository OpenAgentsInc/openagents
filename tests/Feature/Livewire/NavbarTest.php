<?php

use App\Livewire\Navbar;
use App\Models\User;
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

it('shows login button if unauthed', function () {
    $this->get('/')
        ->assertStatus(200)
        ->assertSee('Login');
});

it('does not show login & register buttons if authed', function () {
    $user = User::factory()->create();
    $this->actingAs($user)
        ->get('/')
        ->assertStatus(200)
        ->assertDontSee('Login')
        ->assertDontSee('Register');
});

it('shows a chat button if authed', function () {
    $user = User::factory()->create();
    $this->actingAs($user)
        ->get('/')
        ->assertStatus(200)
        ->assertSee('Chat');
})->skip();

it('does not show a chat button if unauthed', function () {
    $this->get('/')
        ->assertStatus(200)
        ->assertDontSee('Chat');
});
