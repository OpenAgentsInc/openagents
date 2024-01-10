<?php

use App\Models\Agent;
use App\Models\Conversation;
use App\Models\File;
use App\Models\Message;

it('has many messages', function () {
    $conversation = Conversation::factory()->create();
    $message = Message::factory()->create(['conversation_id' => $conversation->id]);

    $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $conversation->messages);
    $this->assertInstanceOf(Message::class, $conversation->messages->first());
});

it('must belong to an agent', function () {
    $agent = Agent::factory()->create();
    $conversation = Conversation::factory()->create([
        'agent_id' => $agent->id,
    ]);

    $this->assertInstanceOf(Agent::class, $conversation->agent);

    $this->expectException(\Illuminate\Database\QueryException::class);
    $conversationNull = Conversation::factory()->create([
        'agent_id' => null,
    ]);
});
