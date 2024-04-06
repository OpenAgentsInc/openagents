<?php

use App\Models\Message;
use App\Models\Thread;
use App\Models\User;

// Basics
it('can be created', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create(['user_id' => $user->id]);
    $this->assertModelExists($thread);
});

// Relationships
it('has many messages', function () {
    $thread = Thread::factory()->create();
    Message::factory(2)->create(['thread_id' => $thread->id]);
    expect($thread->messages)->toHaveCount(2);
});

it('belongs to a user', function () {
    $user = User::factory()->create();
    $thread = Thread::factory()->create(['user_id' => $user->id]);

    $this->assertTrue($thread->user()->exists());
    $this->assertEquals($user->id, $thread->user->id);
});
