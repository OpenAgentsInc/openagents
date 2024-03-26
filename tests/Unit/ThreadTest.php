<?php

use App\Models\Agent;
use App\Models\Message;
use App\Models\Thread;
use App\Models\User;

it('can be created', function () {
    $thread = Thread::factory()->create();
    $this->assertModelExists($thread);
});

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
