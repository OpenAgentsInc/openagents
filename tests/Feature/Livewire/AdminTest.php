<?php

use App\Livewire\Admin;
use App\Models\User;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::actingAs(User::factory()->create(['username' => 'AtlantisPleb']))
        ->test(Admin::class)
        ->assertStatus(200);
});

// redirects to homepage if user is not admin
it('redirects to homepage if user is not admin', function () {
    Livewire::actingAs(User::factory()->create())
        ->test(Admin::class)
        ->assertRedirect(route('home'));
});
