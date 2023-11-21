<?php

use App\Models\Conversation;
use App\Models\File;
use App\Models\Message;

IT('HAS A BODY', function () {
  $message = Message::factory()->create(['body' => 'Hello, world!']);

  $this->assertEquals('Hello, world!', $message->body);
});

IT('HAS A SENDER OF USER OR AGENT', function () {
  $message = Message::factory()->create(['sender' => 'user']);

  $this->assertEquals('user', $message->sender);
});
