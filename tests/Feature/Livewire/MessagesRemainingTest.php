<?php

use App\Livewire\MessagesRemaining;
use App\Models\User;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(MessagesRemaining::class)
        ->assertStatus(200)
        ->assertSeeHtml('You have <span class="text-white">5</span> free responses remaining.')
        ->assertSee('Sign up to get 10 messages every day.');
});

it('renders correctly for authenticated user', function () {
    $this->actingAs(User::factory()->create());

    Livewire::test(MessagesRemaining::class)
        ->assertStatus(200)
        ->assertSeeHtml('You have <span class="text-white">10</span> responses remaining today.')
        ->assertSee('Upgrade to Pro and get 100 responses per day.');
});
