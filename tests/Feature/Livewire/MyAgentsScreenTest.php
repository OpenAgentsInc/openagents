<?php

use App\Livewire\MyAgentsScreen;
use App\Models\User;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::actingAs(User::factory()->create())
        ->test(MyAgentsScreen::class)
        ->assertStatus(200);
});
