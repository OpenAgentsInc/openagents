<?php

use App\Models\Agent;
use App\Models\Step;
use App\Models\Task;

it('belongs to an agent', function () {
  $step = Step::factory()->create();
  expect($step->agent)->toBeInstanceOf(Agent::class);
});

it('belongs to a task', function () {
  $step = Step::factory()->create();
  expect($step->task)->toBeInstanceOf(Task::class);
});

it('has input and output fields', function () {
  $input = ['foo' => 'bar'];
  $output = ['result' => 'success'];

  $step = Step::factory()->create([
    'input' => json_encode($input),
    'output' => json_encode($output)
  ]);

  expect($step->input)->toBe(json_encode($input));
  expect($step->output)->toBe(json_encode($output));
});
