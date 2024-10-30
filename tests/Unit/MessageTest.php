<?php

use App\Models\Message;
use App\Models\User;
use App\Models\Thread;
use App\Models\ToolInvocation;

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
    $message = Message::factory()->system()->create([
        'thread_id' => $thread->id,
    ]);

    expect($message->user)->toBeNull();
    expect($message->thread)->toBeInstanceOf(Thread::class);
    expect($message->role)->toBe('assistant');
});

test('a message can have many tool invocations', function () {
    $message = Message::factory()->create();
    
    // Create multiple tool invocations for the message
    $toolInvocations = ToolInvocation::factory()->count(3)->create([
        'message_id' => $message->id
    ]);

    expect($message->toolInvocations)->toHaveCount(3);
    expect($message->toolInvocations->first())->toBeInstanceOf(ToolInvocation::class);
});