<?php

use App\Livewire\Logs;
use App\Models\User;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::actingAs(User::factory()->create(['username' => 'AtlantisPleb']))
        ->test(Logs::class)
        ->assertStatus(200);
});

// It redirects to the homepage if the user is not isAdmin
it('redirects to the homepage if the user is not isAdmin', function () {
    Livewire::actingAs(User::factory()->create())
        ->test(Logs::class)
        ->assertRedirect(route('home'));
});
