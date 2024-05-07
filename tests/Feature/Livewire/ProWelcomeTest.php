<?php

use App\Livewire\ProWelcome;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(ProWelcome::class)
        ->assertStatus(200);
});
