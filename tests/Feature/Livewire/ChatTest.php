<?php

use App\Livewire\Chat;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Chat::class)
        ->assertStatus(200);
});

it('shows at the chat route /chat', function () {
    $this->get('/chat')
        ->assertStatus(200)
        ->assertSeeLivewire('chat');
});
