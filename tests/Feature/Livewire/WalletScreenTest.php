<?php

use App\Livewire\WalletScreen;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(WalletScreen::class)
        ->assertStatus(200);
});
