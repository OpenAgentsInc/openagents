<?php

use App\Models\Agent;
use App\Models\Message;
use App\Models\Thread;
use App\Models\User;

// Basics
it('can be created', function () {
    $thread = Thread::factory()->create();
    $this->assertModelExists($thread);
});

// Relationships
it('has many messages', function () {
    $thread = Thread::factory()->create();
    Message::factory(2)->create(['thread_id' => $thread->id]);
    expect($thread->messages)->toHaveCount(2);
});

it('belongs to many agents', function () {
    $thread = Thread::factory()->create();
    $agent = Agent::factory()->create();
    $thread->agents()->attach($agent);

    expect($thread->agents)->toHaveCount(1);
});

it('belongs to many users', function () {
    $thread = Thread::factory()->create();
    $user = User::factory()->create();
    $thread->users()->attach($user);

    expect($thread->users)->toHaveCount(1);
});

// Privacy
it('can be private', function () {
    $thread = Thread::factory()->private()->create();
    expect($thread->private)->toBeTrue();
});

it('is public by default', function () {
    $thread = Thread::factory()->create();
    expect($thread->private)->toBeFalse();
});

it('can be set private', function () {
    $thread = Thread::factory()->create();
    $thread->setPrivate();
    expect($thread->private)->toBeTrue();
});

it('can be set public', function () {
    $thread = Thread::factory()->private()->create();
    $thread->setPublic();
    expect($thread->private)->toBeFalse();
});
