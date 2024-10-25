<?php

use App\Models\Message;
use App\Models\User;
use App\Models\Thread;

test('a message belongs to a user', function () {
    $user = User::factory()->create();
    $message = Message::factory()->create(['user_id' => $user->id]);

    expect($message->user)->toBeInstanceOf(User::class);
    expect($message->user->id)->toBe($user->id);
});

test('a message belongs to a thread', function () {
    $thread = Thread::factory()->create();
    $message = Message::factory()->create(['thread_id' => $thread->id]);

    expect($message->thread)->toBeInstanceOf(Thread::class);
    expect($message->thread->id)->toBe($thread->id);
});

test('a message can be created by the system', function () {
    $thread = Thread::factory()->create();
    $message = Message::factory()->create([
        'thread_id' => $thread->id,
        'user_id' => null,
        'is_system_message' => true,
    ]);

    expect($message->user)->toBeNull();
    expect($message->is_system_message)->toBeTrue();
    expect($message->thread)->toBeInstanceOf(Thread::class);
});