<?php

use App\Models\Agent;
use App\Models\Artifact;
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

it('has many artifacts', function () {
    $agent = Agent::factory()->create();
    $task = Task::factory()->create();
    $task->artifacts()->create([
      'name' => 'foo',
      'path' => 'bar',
      'agent_id' => $agent->id,
    ]);
    $task->artifacts()->create([
      'name' => 'baz',
      'path' => 'qux',
      'agent_id' => $agent->id,
    ]);
    expect($task->artifacts)->toHaveCount(2);
});

it('has proper relationships', function () {
    $agent = Agent::factory()->create();

    $task = Task::factory()->create([
      'agent_id' => $agent->id
    ]);

    expect($task->agent->id)->toBe($agent->id);

    $step = Step::factory()->create([
      'agent_id' => $agent->id,
    //   'task_id' => $task->id
    ]);

    expect($step->agent->id)->toBe($agent->id);
    // expect($step->task->id)->toBe($task->id);

    $artifact = Artifact::factory()->create([
      'agent_id' => $agent->id,
      'task_id' => $task->id
    ]);

    expect($artifact->agent->id)->toBe($agent->id);
    expect($artifact->task->id)->toBe($task->id);
});

it('requires agent_id on create', function () {
    $this->expectException(\Exception::class);
    Task::factory()->create([
      'agent_id' => null
    ]);
});
