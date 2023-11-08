<?php

use App\Models\Agent;
use App\Models\Conversation;
use App\Models\User;

it('has many agents', function () {
  $user = User::factory()->create();
  $agent = Agent::factory()->create(['user_id' => $user->id]);

  $this->assertInstanceOf(
    'Illuminate\Database\Eloquent\Collection',
    $user->agents
  );

  $this->assertInstanceOf(
    'App\Models\Agent',
    $user->agents->first()
  );
});

it('has many conversations', function () {
  $user = User::factory()->create();
  $conversation = Conversation::factory()->create(['user_id' => $user->id]);

  $this->assertInstanceOf(
    'Illuminate\Database\Eloquent\Collection',
    $user->conversations
  );

  $this->assertInstanceOf(
    'App\Models\Conversation',
    $user->conversations->first()
  );
});
