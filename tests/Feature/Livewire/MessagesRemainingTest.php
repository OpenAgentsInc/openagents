<?php

use App\Livewire\MessagesRemaining;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(MessagesRemaining::class)
        ->assertStatus(200);
});
