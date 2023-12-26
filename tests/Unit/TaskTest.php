<?php

use App\Models\Agent;
use App\Models\Step;
use App\Models\Task;

it('has a description', function () {
    $task = Task::factory()->create([
      'description' => null,
    ]);
    expect($task->description)->toBeNull();

    $task = Task::factory()->create([
      'description' => 'foo',
    ]);
    expect($task->description)->toBe('foo');
});

it('has one output', function () {
    $task = Task::factory()->create();
    expect($task->output)->toBeNull();

    $task = Task::factory()->create([
      'output' => json_encode(['foo' => 'bar'])
    ]);
    expect($task->output)->toBe(json_encode(['foo' => 'bar']));
});

it('belongs to an agent', function () {
    $task = Task::factory()->create();
    expect($task->agent)->toBeInstanceOf(Agent::class);
});

it('has proper relationships', function () {
    $agent = Agent::factory()->create();

    $task = Task::factory()->create([
      'agent_id' => $agent->id
    ]);

    expect($task->agent->id)->toBe($agent->id);

    $step = Step::factory()->create([
      'agent_id' => $agent->id,
    ]);

    expect($step->agent->id)->toBe($agent->id);
});

it('requires agent_id on create', function () {
    $this->expectException(\Exception::class);
    Task::factory()->create(['agent_id' => null]);
});

it('has many steps', function () {
    $task = Task::factory()->create();
    Step::factory(2)->create(['task_id' => $task->id]);
    expect($task->steps)->toHaveCount(2);
});
