<?php

use App\Livewire\ProWelcome;
use App\Models\User;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::actingAs(User::factory()->create())
        ->test(ProWelcome::class)
        ->assertStatus(200);
});
