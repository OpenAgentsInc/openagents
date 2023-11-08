<?php

use App\Models\Conversation;
use App\Models\File;
use App\Models\Message;

it('has many messages', function () {
  $conversation = Conversation::factory()->create();
  $message = Message::factory()->create(['conversation_id' => $conversation->id]);

  $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $conversation->messages);
  $this->assertInstanceOf(Message::class, $conversation->messages->first());
});

it('has many files', function () {
  $conversation = Conversation::factory()->create();
  $file = File::factory()->create(['conversation_id' => $conversation->id]);

  $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $conversation->files);
  $this->assertInstanceOf(File::class, $conversation->files->first());
});
