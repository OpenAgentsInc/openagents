<?php

use App\Livewire\PrismDashboard;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(PrismDashboard::class)
        ->assertStatus(200);
});
