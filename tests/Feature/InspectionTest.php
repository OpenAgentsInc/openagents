<?php

use App\Models\Agent;
use App\Models\Step;
use App\Models\Task;

test('guest can visit inspection dashboard and see all agents: tasks & steps', function () {
    $agent = Agent::factory()->create();
    $task = Task::factory()->create([
        'agent_id' => $agent->id,
        'prompt' => "Make a pull request that solves a GitHub issue",
    ]);
    $step = Step::factory()->create([
        'task_id' => $task->id,
        'input' => json_encode([
            "type" => "llm",
            "model" => "gpt-4",
            "instruction" => "Make a plan to solve the issue"
        ]),
        'output' => json_encode([
            "response" => "1. Do this\n2. Do that\n3. Do the other thing",
            "tokens_used" => 1234,
        ]),
    ]);
    $step2 = Step::factory()->create([
        'task_id' => $task->id,
        'input' => json_encode([
            "type" => "vector_query",
            "instruction" => "Relevant query based on etc"
        ]),
        'output' => json_encode([
            "response" => "bla bla bla",
            "tokens_used" => 123344,
        ]),
    ]);

    $response = $this->get('/inspect');

    // assert see all agents/tasks/steps - and json_encode/decode to check input/output
    $step1input = json_decode($step->input);
    $step1output = json_decode($step->output);
    $step2input = json_decode($step2->input);
    $step2output = json_decode($step2->output);

    $response->assertStatus(200)
        ->assertSee($agent->name)
        ->assertSee($task->prompt)
        ->assertSee($step1input->type)
        ->assertSee($step1input->model)
        ->assertSee($step1input->instruction)
        ->assertSee($step1output->response)
        ->assertSee($step1output->tokens_used)
        ->assertSee($step2input->type)
        ->assertSee($step2input->instruction)
        ->assertSee($step2output->response)
        ->assertSee($step2output->tokens_used);
});

// can visit task run page and see all steps taken
// can click on any step to see full details of input/output/metadata

// later: agent owner can modify prompts used
