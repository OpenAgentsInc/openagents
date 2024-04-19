<?php

use App\Livewire\Settings;
use App\Models\User;
use Livewire\Livewire;

it('renders successfully', function () {
    $this->actingAs(User::factory()->create());

    Livewire::test(Settings::class)
        ->assertStatus(200);
});
