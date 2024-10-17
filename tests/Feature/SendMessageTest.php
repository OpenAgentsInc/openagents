<?php

use App\Models\User;
use Illuminate\Foundation\Testing\RefreshDatabase;

uses(RefreshDatabase::class);

test('authenticated user can send a message', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)
        ->post('/send-message', [
            'message' => 'Test message'
        ]);

    $response->assertStatus(302);
    $response->assertSessionHas('success', 'Message sent successfully!');

    $this->assertDatabaseHas('messages', [
        'user_id' => $user->id,
        'content' => 'Test message'
    ]);
});

test('unauthenticated user cannot send a message', function () {
    $response = $this->post('/send-message', [
        'message' => 'Test message'
    ]);

    $response->assertStatus(302);
    $response->assertRedirect('/login');
});

test('message cannot be empty', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)
        ->post('/send-message', [
            'message' => ''
        ]);

    $response->assertStatus(302);
    $response->assertSessionHasErrors('message');
});