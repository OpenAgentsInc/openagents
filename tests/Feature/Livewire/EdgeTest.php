<?php

use App\Livewire\Edge;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Edge::class)
        ->assertStatus(200);
})->skip();
