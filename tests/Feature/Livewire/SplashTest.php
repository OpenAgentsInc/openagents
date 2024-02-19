<?php

use App\Livewire\Splash;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Splash::class)
        ->assertStatus(200);
});

it('shows at the home route /', function () {
    $this->get('/')
        ->assertStatus(200)
        ->assertSeeLivewire('splash');
});
