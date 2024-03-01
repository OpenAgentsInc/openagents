<?php

use App\Livewire\Chat;
use App\Models\Agent;
use App\Models\Conversation;
use App\Models\User;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Chat::class)
        ->assertStatus(200);
})->skip();

test('sending message requires an authed user', function () {
    Livewire::test(Chat::class)
        ->set('body', 'Hello')
        ->call('sendMessage')
        ->assertForbidden();
})->skip();

it('shows at the chat route /chat', function () {
    $this->get('/chat')
        ->assertStatus(200)
        ->assertSeeLivewire('chat');
})->skip();

it('loads messages', function () {
    Livewire::test(Chat::class)
        ->assertSet('messages', []);
})->skip();

it('shows messages', function () {
    Livewire::test(Chat::class)
        ->set('messages', [
            ['body' => 'Hello', 'sender' => 'You'],
            ['body' => 'Hi', 'sender' => 'Agent'],
        ])
        ->assertSee('Hello')
        ->assertSee('Hi');
})->skip();

it('shows conversations on sidebar', function () {
    Conversation::factory()->create(['title' => 'John Doe']);
    Conversation::factory()->create(['title' => 'Jane Doe']);

    Livewire::test(Chat::class)
        ->assertSee('John Doe')
        ->assertSee('Jane Doe');
})->skip();

it('creates and sets conversation on message in new chat', function () {
    $user = User::factory()->create();
    $this->actingAs($user);
    Agent::factory()->create(['user_id' => $user->id]);

    Livewire::test(Chat::class)
        ->set('body', 'Hello')
        ->call('sendMessage')
        ->assertSet('conversation.id', Conversation::first()->id);

    expect(Conversation::count())->toBe(1);
})->skip();
