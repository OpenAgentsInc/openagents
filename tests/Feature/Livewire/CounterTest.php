<?php

use App\Livewire\Counter;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Counter::class)
        ->assertStatus(200);
});
