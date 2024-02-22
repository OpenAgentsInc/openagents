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

it('loads messages', function () {
    Livewire::test(Chat::class)
        ->assertSet('messages', []);
});

it('shows messages', function () {
    Livewire::test(Chat::class)
        ->set('messages', [
            ['body' => 'Hello', 'from' => 'You'],
            ['body' => 'Hi', 'from' => 'Agent'],
        ])
        ->assertSee('Hello')
        ->assertSee('Hi');
});
