<?php

use App\Models\Agent;
use App\Models\Conversation;
use App\Models\User;
use App\Livewire\Chat;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Chat::class)
        ->assertStatus(200);
});

test('sending message requires an authed user', function () {
    Livewire::test(Chat::class)
        ->set('body', 'Hello')
        ->call('sendMessage')
        ->assertForbidden();
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
            ['body' => 'Hello', 'sender' => 'You'],
            ['body' => 'Hi', 'sender' => 'Agent'],
        ])
        ->assertSee('Hello')
        ->assertSee('Hi');
});

it('shows conversations on sidebar', function () {
    Conversation::factory()->create(['title' => 'John Doe']);
    Conversation::factory()->create(['title' => 'Jane Doe']);

    Livewire::test(Chat::class)
        ->assertSee('John Doe')
        ->assertSee('Jane Doe');
});

it('creates and sets conversation on message in new chat', function () {
    $user = User::factory()->create();
    $this->actingAs($user);
    Agent::factory()->create(['user_id' => $user->id]);

    Livewire::test(Chat::class)
        ->set('body', 'Hello')
        ->call('sendMessage')
        ->assertSet('conversation.id', Conversation::first()->id);

    expect(Conversation::count())->toBe(1);
});
