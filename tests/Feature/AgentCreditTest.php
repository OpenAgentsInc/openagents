<?php

use App\Livewire\Chat;
use App\Models\Agent;
use App\Models\User;
use App\Services\AgentChatService;
use Livewire\Livewire;

test('interacting with agent deducts credit', function () {
    // Given we have a user with some credit
    $user = User::factory()->create(['credits' => 5]);
    $this->actingAs($user);

    // Given this user owns a Thread
    $thread = $user->threads()->create();

    // Given there is an agent
    $agent = Agent::factory()->create([
        'name' => 'GitHub Repo Analyzer',
        'about' => 'Feed me a GitHub repo link and I will analyze it for you',
        'message' => 'You are a helpful assistant who knows how to analyze GitHub repositories. Users may use you to analyze repositories and provide insights on the codebase.',
        'prompt' => "Hello! Drop a link to a repo and I'll look at it for ya",
    ]);

    // User visits Chat page
    Livewire::test(Chat::class, ['id' => $thread->id])
        ->assertStatus(200)
        ->set('message_input', 'Hello world')
        ->call('sendMessage');

    // Make the call that will otherwise happen in the Chat js part for chat with agents
    $agentChatService = new AgentChatService();
    $success = $agentChatService->chatWithAgent($thread, $agent, 'Hello world');

    // Assert that succeeded
    $this->assertTrue($success);

    // Assert that the user's agent credit was deducted
    $this->assertEquals(4, $user->credits);
});

test('no interaction possible with agent with no credit', function () {
    // Given we have a user with no credit
    $user = User::factory()->create(['credits' => 0]);
    $this->actingAs($user);

    // Given this user owns a Thread
    $thread = $user->threads()->create();

    // Given there is an agent
    $agent = Agent::factory()->create([
        'name' => 'GitHub Repo Analyzer',
        'about' => 'Feed me a GitHub repo link and I will analyze it for you',
        'message' => 'You are a helpful assistant who knows how to analyze GitHub repositories. Users may use you to analyze repositories and provide insights on the codebase.',
        'prompt' => "Hello! Drop a link to a repo and I'll look at it for ya",
    ]);

    // User visits Chat page
    Livewire::test(Chat::class, ['id' => $thread->id])
        ->assertStatus(200)
        ->set('message_input', 'Hello world')
        ->call('sendMessage');

    // Make the call that will otherwise happen in the Chat js part for chat with agents
    $agentChatService = new AgentChatService();
    $success = $agentChatService->chatWithAgent($thread, $agent, 'Hello world');

    // Assert that failed
    $this->assertFalse($success);
});
