<?php

use App\Livewire\Changelog;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Changelog::class)
        ->assertStatus(200);
});
