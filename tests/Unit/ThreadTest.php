<?php

use App\Models\Message;
use App\Models\Thread;

it('can be created', function () {
    $thread = Thread::factory()->create();
    $this->assertModelExists($thread);
});

it('has many messages', function () {
    $thread = Thread::factory()->create();
    Message::factory(2)->create(['thread_id' => $thread->id]);
    expect($thread->messages)->toHaveCount(2);
});
