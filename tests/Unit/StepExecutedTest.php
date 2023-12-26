<?php

use App\Models\Step;
use App\Models\StepExecuted;
use App\Models\TaskExecuted;
use App\Models\User;

it('can run', function () {
    $step_executed = StepExecuted::factory()->create();
    $step_executed->run();
});

it('has an order', function () {
    $step_executed = StepExecuted::factory()->create(['order' => 1]);
    expect($step_executed->order)->toBe(1);
});

it('has a status', function () {
    $step_executed = StepExecuted::factory()->create(['status' => 'pending']);
    expect($step_executed->status)->toBe('pending');

    $step_executed = StepExecuted::factory()->create(['status' => 'success']);
    expect($step_executed->status)->toBe('success');
});

it('belongs to a step', function () {
    $step_executed = StepExecuted::factory()->create();
    expect($step_executed->step)->toBeInstanceOf(Step::class);
});

it('belongs to an executed task', function () {
    $step_executed = StepExecuted::factory()->create();
    expect($step_executed->task_executed)->toBeInstanceOf(TaskExecuted::class);
});

it('belongs to a user', function () {
    $step_executed = StepExecuted::factory()->create();
    expect($step_executed->user)->toBeInstanceOf(User::class);
});

it('has optional input and output', function () {
    $step = StepExecuted::factory()->create([
        'input' => null,
        'output' => null
    ]);
    expect($step->input)->toBeNull();
    expect($step->output)->toBeNull();

    $input = ['foo' => 'bar'];
    $output = ['result' => 'success'];

    $step = StepExecuted::factory()->create([
      'input' => json_encode($input),
      'output' => json_encode($output)
    ]);

    expect($step->input)->toBe(json_encode($input));
    expect($step->output)->toBe(json_encode($output));
});

it('has optional params', function () {
    $step_executed = StepExecuted::factory()->create(['params' => null]);
    expect($step_executed->params)->toBeNull();

    $step_executed = StepExecuted::factory()->create(['params' => json_encode(['temperature' => 0.1])]);
    expect(json_decode($step_executed->params)->temperature)->toBe(0.1);
});
