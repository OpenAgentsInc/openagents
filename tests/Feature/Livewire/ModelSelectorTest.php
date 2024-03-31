<?php

use App\Livewire\ModelSelector;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(ModelSelector::class)
        ->assertStatus(200);
});
