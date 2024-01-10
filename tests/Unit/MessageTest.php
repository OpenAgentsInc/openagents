<?php

use App\Models\Conversation;
use App\Models\File;
use App\Models\Message;

it('has a body', function () {
    $message = Message::factory()->create(['body' => 'Hello, world!']);

    $this->assertEquals('Hello, world!', $message->body);
});

it('has a sender of user or agent', function () {
    $message = Message::factory()->create(['sender' => 'user']);

    $this->assertEquals('user', $message->sender);
});

it('may have a user', function () {
    $message = Message::factory()->create(['user_id' => null]);

    $this->assertNull($message->user);

    $message = Message::factory()->create();

    $this->assertNotNull($message->user);
});
