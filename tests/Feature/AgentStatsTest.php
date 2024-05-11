<?php

use App\Models\Agent;
use App\Models\Thread;
use App\Models\User;

test('can retrieve number of threads for an agent', function () {
    // Create 5 threads
    $threads = Thread::factory(5)->create();

    // Create 5 users
    $users = User::factory(5)->create();

    // Create 5 agents
    $agents = Agent::factory(5)->create();

    // Create 5 messages for each thread
    foreach ($threads as $thread) {
        foreach ($users as $user) {
            foreach ($agents as $agent) {
                $thread->messages()->create([
                    'user_id' => $user->id,
                    'agent_id' => $agent->id,
                    'body' => 'Hello, world!',
                ]);
            }
        }
    }

    // Get 5 random agents from the created agents
    $randomAgents = $agents->random(5);

    // Calculate the expected number of threads for each random agent
    $expectedThreadCount = $threads->count();

    // Assert the number of threads for each random agent using the thread_count attribute
    foreach ($randomAgents as $agent) {
        $this->assertEquals($expectedThreadCount, $agent->thread_count);
    }
});
