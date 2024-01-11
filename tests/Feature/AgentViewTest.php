<?php

use App\Models\Agent;
use App\Models\Conversation;
use App\Models\File;
use App\Models\Message;
use Database\Seeders\ConciergeSeeder;
use Inertia\Testing\AssertableInertia as Assert;

test('guest sees view agent page', function () {
    $this->seed(ConciergeSeeder::class);

    $agentId = Agent::first()->id;

    $response = $this->get('/agent/' . $agentId);

    $response
        ->assertInertia(
            fn (Assert $page) => $page
            ->component('AgentView')
        );
});

test('agent view knows its authors username', function () {
    $this->seed(ConciergeSeeder::class);

    $agent = Agent::first();

    $response = $this->get('/agent/' . $agent->id);

    $response
        ->assertInertia(
            fn (Assert $page) => $page
            ->component('AgentView')
            ->has('owner')
            ->where('owner', $agent->user->username)
        );
});

test('agent view knows the files associated with the agent', function () {
    $this->seed(ConciergeSeeder::class);

    $agent = Agent::first();
    File::factory()->create([
        'agent_id' => $agent->id,
        'user_id' => $agent->user_id,
        'name' => 'test.pdf',
        'path' => 'test.pdf',
        'size' => 1000,
    ]);

    $response = $this->get('/agent/' . $agent->id);

    $response
        ->assertInertia(
            fn (Assert $page) => $page
            ->component('AgentView')
            ->has('files')
            ->where('files.0.name', 'test.pdf')
        );
});

test('agent view knows the previous conversation', function () {
    $this->seed(ConciergeSeeder::class);

    $agent = Agent::first();

    $user = $agent->user;
    $this->actingAs($user);

    $conversation = Conversation::factory()->create([
        'agent_id' => $agent->id,
        'user_id' => $agent->user_id
    ]);

    Message::factory(3)->create([
        'conversation_id' => $conversation->id,
        'user_id' => $agent->user_id
    ]);

    $response = $this->get('/agent/' . $agent->id);

    $response
        ->assertInertia(
            fn (Assert $page) => $page
            ->component('AgentView')
            ->has('conversation')
            ->where('conversation.messages.0.body', $conversation->messages()->latest()->first()->body)
        );
});

test('previous conversation does not include messages with no user_id', function () {
    $this->seed(ConciergeSeeder::class);

    $agent = Agent::first();

    $conversation = Conversation::factory()->create([
        'agent_id' => $agent->id,
        'user_id' => $agent->user_id
    ]);

    Message::factory()->create([
        'conversation_id' => $conversation->id,
        'user_id' => $agent->user_id
    ]);

    $nullMessage = Message::factory()->create([
        'conversation_id' => $conversation->id,
        'user_id' => null
    ]);

    $response = $this->get('/agent/' . $agent->id);

    $response
        ->assertInertia(
            fn (Assert $page) => $page
            ->component('AgentView')
            ->has('conversation')
            ->whereNot('conversation.messages', $nullMessage->toArray()['body'])
        );
});

test('redirect epstein/ to agent 2', function () {
    Agent::factory(2)->create();

    $response = $this->get('/agent/epstein');

    $response->assertRedirect('/agent/2');
});
