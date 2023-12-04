<?php

use App\Models\Agent;
use App\Models\Step;
use App\Models\Task;
use Database\Seeders\DatabaseSeeder;

test('guest can visit inspection dashboard and see all agents: tasks & steps', function () {
    $this->seed(DatabaseSeeder::class);

    $response = $this->get('/inspect');

    // You might need to retrieve some instances to assert against
    $agent = Agent::first();
    $task = Task::first();
    $step = Step::first();
    $stepInput = json_decode($step->input);
    $stepOutput = json_decode($step->output);

    // assert see all agents/tasks/steps
    $response->assertStatus(200)
        ->assertSee($agent->name)
        ->assertSee($task->prompt)
        ->assertSee($stepInput->type)
        ->assertSee($stepInput->model ?? '')
        ->assertSee($stepInput->instruction)
        ->assertSee($stepOutput->response)
        ->assertSee($stepOutput->tokens_used);
});

test('can visit task run page and see all steps taken', function () {
    $this->seed(DatabaseSeeder::class);

    $task = Task::first();
    $steps = $task->steps;

    $response = $this->get("/inspect/{$task->id}");

    $response->assertStatus(200);

    foreach ($steps as $step) {
        $stepInput = json_decode($step->input);
        $stepOutput = json_decode($step->output);

        $response->assertSee($stepInput->type)
            ->assertSee($stepInput->model ?? '')
            ->assertSee($stepInput->instruction)
            ->assertSee($stepOutput->response)
            ->assertSee($stepOutput->tokens_used);
    }
});

// can click on any step to see full details of input/output/metadata

// later: agent owner can modify prompts used
