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

    $prompt = 'Hello world!';
    // User visits Chat page
    Livewire::test(Chat::class, ['id' => $thread->id])
        ->assertStatus(200)
        ->assertSeeHtml('>How can we help you today?</h3>')
        ->set('message_input', $prompt)
        ->call('sendMessage')
        ->assertSeeHtml('mistral.png" alt="Model Image">')
        ->assertSeeHtml('>You</span>')
        ->assertSeeHtml("markdown-content prompt\"><p>$prompt</p>");
});
