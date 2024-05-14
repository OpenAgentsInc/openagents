<?php

use App\Livewire\Logs;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Logs::class)
        ->assertStatus(200);
});
