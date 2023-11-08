<?php

use App\Models\Conversation;
use App\Models\Message;

it('has many messages', function () {
  $conversation = Conversation::factory()->create();
  $message = Message::factory()->create(['conversation_id' => $conversation->id]);

  $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $conversation->messages);
  $this->assertInstanceOf(Message::class, $conversation->messages->first());
});
