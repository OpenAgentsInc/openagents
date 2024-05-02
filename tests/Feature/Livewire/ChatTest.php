<?php

use App\Livewire\Chat;
use App\Models\User;
use Livewire\Livewire;

test('you can chat with a model', function () {
    // Given we have a user with no credit
    $user = User::factory()->create(['credits' => 0]);
    $this->actingAs($user);

    // Given this user owns a Thread
    $thread = $user->threads()->create();

    // User visits Chat page
    Livewire::test(Chat::class, ['id' => $thread->id])
        ->assertStatus(200)
        ->set('message_input', 'Hello world')
        ->call('sendMessage');

    // That's as far as we can get testing like this because tests don't execute the ->js part
})->skip(); // unfuck this later
