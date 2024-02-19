<?php

use App\Livewire\Graph;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Graph::class)
        ->assertStatus(200);
});
