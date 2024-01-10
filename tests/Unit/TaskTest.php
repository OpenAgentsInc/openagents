<?php

use App\Models\Agent;
use App\Models\Step;
use App\Models\Task;
use App\Models\TaskExecuted;

it('can run', function () {
    $task = Task::factory()->create();
    $task->run();

    expect(TaskExecuted::count())->toBe(1);
});

it('may have a name', function () {
    $task = Task::factory()->create([
      'name' => null,
    ]);
    expect($task->name)->toBeNull();

    $task = Task::factory()->create([
      'name' => 'foo',
    ]);
    expect($task->name)->toBe('foo');
});

it('may have a description', function () {
    $task = Task::factory()->create([
      'description' => null,
    ]);
    expect($task->description)->toBeNull();

    $task = Task::factory()->create([
      'description' => 'foo',
    ]);
    expect($task->description)->toBe('foo');
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
