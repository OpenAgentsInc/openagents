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

test('can retrieve number of unique users for an agent', function () {
    // Create 5 threads
    $threads = Thread::factory(5)->create();

    // Create 5 users
    $users = User::factory(5)->create();

    // Create 5 agents
    $agents = Agent::factory(5)->create();

    // Create messages for each thread with different user and agent combinations
    foreach ($threads as $thread) {
        foreach ($users as $user) {
            foreach ($agents as $agent) {
                // Randomly decide whether to create a message for this combination
                if (rand(0, 1)) {
                    $thread->messages()->create([
                        'user_id' => $user->id,
                        'agent_id' => $agent->id,
                        'body' => 'Hello, world!',
                    ]);
                }
            }
        }
    }

    // Get 5 random agents from the created agents
    $randomAgents = $agents->random(5);

    // Assert the number of unique users for each random agent using the unique_users_count attribute
    foreach ($randomAgents as $agent) {
        $expectedUniqueUsersCount = $agent->messages()->distinct('user_id')->count('user_id');
        $this->assertEquals($expectedUniqueUsersCount, $agent->unique_users_count);
    }
});
