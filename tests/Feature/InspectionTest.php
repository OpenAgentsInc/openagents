<?php

use App\Models\Agent;
use App\Models\Step;
use App\Models\Task;
use Database\Seeders\DatabaseSeeder;
use Inertia\Testing\AssertableInertia as Assert;

test('guest can visit inspection dashboard and see all agents: tasks & steps', function () {
    $this->seed(DatabaseSeeder::class);

    $response = $this->get('/inspect');

    $agentCount = Agent::count();

    $agent = Agent::first();
    // dd($agent);
    $task = Task::first();
    $step = Step::first();
    $stepInput = json_decode($step->input);
    $stepOutput = json_decode($step->output);

    $response->assertStatus(200)
        ->assertInertia(
            fn (Assert $page) => $page
            ->component('Inspect') // Replace with your actual component name
            ->has('agents', $agentCount, fn (Assert $page) => $page
                ->where('name', $agent->name) // Adjust based on actual structure
                ->has('tasks', $agent->tasks->count(), fn (Assert $page) => $page
                    ->where('description', $task->description) // Adjust based on actual structure
                    // ->where('steps', $task->steps->count(), fn (Assert $page) => $page
                    //     ->where('type', $stepInput->type) // Adjust based on actual structure
                    //     ->where('model', $stepInput->model ?? '') // Adjust based on actual structure
                    //     ->where('instruction', $stepInput->instruction) // Adjust based on actual structure
                    //     // ->where('response', $stepOutput->response) // Adjust based on actual structure
                    //     // ->where('tokens_used', $stepOutput->tokens_used) // Adjust based on actual structure
                    // ))
                )
                    ->etc()
                ->etc()
                // ... other assertions for agents
            )
        );

    // $response->assertStatus(200)
    //     ->assertSee($agent->name)
    //     ->assertSee($task->description)
    //     ->assertSee($stepInput->type)
    //     ->assertSee($stepInput->model ?? '')
    //     ->assertSee($stepInput->instruction);
    // ->assertSee($stepOutput->response)
    // ->assertSee($stepOutput->tokens_used);
});

test('can visit task run page and see all steps taken', function () {
    $this->seed(DatabaseSeeder::class);

    $task = Task::first();
    $steps = $task->steps;

    $response = $this->get("/task/{$task->id}");

    $response->assertStatus(200);

    foreach ($steps as $step) {
        $stepInput = json_decode($step->input);
        $stepOutput = json_decode($step->output);

        $response->assertSee($stepInput->type)
            ->assertSee($stepInput->model ?? '')
            ->assertSee($stepInput->instruction);
        // ->assertSee($stepOutput->response)
        // ->assertSee($stepOutput->tokens_used);
    }
});

test('can click on any step to see full details of input/output/metadata', function () {
    $this->seed(DatabaseSeeder::class);

    $step = Step::first();
    $stepInput = json_decode($step->input);
    $stepOutput = json_decode($step->output);

    $response = $this->get("/step/{$step->id}");

    $response->assertStatus(200)
        ->assertSee($stepInput->type)
        ->assertSee($stepInput->model ?? '')
        ->assertSee($stepInput->instruction);
    // ->assertSee($stepOutput->response)
    // ->assertSee($stepOutput->tokens_used);
});

// later: agent owner can modify prompts used
