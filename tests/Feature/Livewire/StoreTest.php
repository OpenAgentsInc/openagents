<?php

use App\Livewire\Store;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Store::class)
        ->assertStatus(200)
        ->assertSee('Discover and create custom agents')
        ->assertSee('Create Agent');
});
