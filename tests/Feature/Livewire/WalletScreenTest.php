<?php

use App\Livewire\WalletScreen;
use App\Models\User;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::actingAs(User::factory()->create())
        ->test(WalletScreen::class)
        ->assertStatus(200);
});
