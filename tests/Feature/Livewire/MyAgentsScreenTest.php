<?php

use App\Livewire\MyAgentsScreen;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(MyAgentsScreen::class)
        ->assertStatus(200);
});
