<?php

use App\Livewire\Settings;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Settings::class)
        ->assertStatus(200);
});
