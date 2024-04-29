<?php

use App\Livewire\Chat;
use App\Models\Agent;
use App\Models\User;
use App\Services\AgentChatService;
use Livewire\Livewire;

test('interacting with agent deducts credit', function () {
    [$user, $thread, $agent] = setupChatScenario(5);

    // Make the call that will otherwise happen in the Chat js part for chat with agents
    $agentChatService = new AgentChatService();
    $success = $agentChatService->chatWithAgent($thread, $agent, 'Hello world');

    // Assert that succeeded
    expect($success)->toBeTrue();

    // Assert that the user's agent credit was deducted
    expect($user->credits)->toBe(4);
});

test('no interaction possible with agent with no credit', function () {
    [$user, $thread, $agent] = setupChatScenario(0);

    // Make the call that will otherwise happen in the Chat js part for chat with agents
    $agentChatService = new AgentChatService();
    $success = $agentChatService->chatWithAgent($thread, $agent, 'Hello world');

    // Assert that failed
    expect($success)->toBeFalse();
});

function setupChatScenario($userCredits)
{
    // Given we have a user with some credit
    $user = User::factory()->create(['credits' => $userCredits]);

    // Given this user owns a Thread
    $thread = $user->threads()->create();

    // Given there is an agent
    $agent = Agent::factory()->create([
        'name' => 'GitHub Repo Analyzer',
        'about' => 'Feed me a GitHub repo link and I will analyze it for you',
        'message' => 'You are a helpful assistant who knows how to analyze GitHub repositories. Users may use you to analyze repositories and provide insights on the codebase.',
        'prompt' => "Hello! Drop a link to a repo and I'll look at it for ya",
    ]);

    Livewire::actingAs($user)
        ->test(Chat::class, ['id' => $thread->id])
        ->assertStatus(200)
        ->set('message_input', 'Hello world')
        ->call('sendMessage');

    return [$user, $thread, $agent];
}
