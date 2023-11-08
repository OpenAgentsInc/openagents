<?php

use App\Models\Agent;
use App\Models\Conversation;
use App\Models\User;

it('has many conversations', function () {
  $user = User::factory()->create();
  $agent = Agent::factory()->create(['user_id' => $user->id]);
  $conversation = Conversation::factory()->create([
    'agent_id' => $agent->id,
    'user_id' => $user->id
  ]);

  $this->assertInstanceOf(
    'Illuminate\Database\Eloquent\Collection',
    $agent->conversations
  );

  $this->assertInstanceOf(
    'App\Models\Conversation',
    $agent->conversations->first()
  );
});
