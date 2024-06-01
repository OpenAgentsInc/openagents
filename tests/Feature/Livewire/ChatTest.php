<?php

use App\Livewire\Chat;
use App\Models\Agent;
use App\Models\PoolJob;
use App\Models\Thread;
use App\Models\User;
use Livewire\Livewire;

test('you can chat with a model', function () {
    // Given we have a user
    $user = User::factory()->create();
    $this->actingAs($user);

    // Given this user owns a Thread
    $thread = $user->threads()->create();

    $prompt = 'Hello world!';
    // User visits Chat page
    Livewire::test(Chat::class, ['id' => $thread->id])
        ->assertStatus(200)
        ->assertSeeHtml('>How can we help you today?</h3>')
        ->set('message_input', $prompt)
        ->call('sendMessage')
        ->assertSeeHtml('mistral.png" alt="Model Image">')
        ->assertSeeHtml('>You</span>')
        ->assertSeeHtml("markdown-content prompt\"><p>$prompt</p>");
});

test('Chat can get agent model from a job', function () {
    $user = User::factory()->create();
    $this->actingAs($user);

    $thread = Thread::factory()->create(['user_id' => $user->id]);
    $agent = Agent::factory()->create([
        'model' => 'mistral-small-latest',
        'pro_model' => 'codestral-latest',
    ]);
    $job = PoolJob::factory()->create([
        'thread_id' => $thread->id,
        'agent_id' => $agent->id,
    ]);

    Livewire::test(Chat::class, ['id' => $thread->id])
        ->call('setAgentModel', $job->toArray())
        ->assertSet('selectedModel', $agent->model);
});

test('Chat can get agent pro model from a job if user is a pro', function () {
    $user = User::factory()->create();
    $this->actingAs($user);

    $thread = Thread::factory()->create(['user_id' => $user->id]);
    $agent = Agent::factory()->create([
        'model' => 'mistral-small-latest',
        'pro_model' => 'codestral-latest',
    ]);

    $job = PoolJob::factory()->create([
        'thread_id' => $thread->id,
        'agent_id' => $agent->id,
    ]);

    Livewire::test(Chat::class, ['id' => $thread->id])
        ->call('setAgentModel', $job->toArray())
        ->assertSet('selectedModel', $agent->pro_model);
})->skip();

test('Chat uses default selectedModel if no agent model is set', function () {
    $user = User::factory()->create();
    $this->actingAs($user);

    $thread = Thread::factory()->create(['user_id' => $user->id]);
    $job = PoolJob::factory()->create(['thread_id' => $thread->id]);

    Livewire::test(Chat::class, ['id' => $thread->id])
        ->call('setAgentModel', $job->toArray())
        ->assertSet('selectedModel', 'command-r-plus');
});
