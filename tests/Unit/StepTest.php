<?php

use App\Models\Agent;
use App\Models\Run;
use App\Models\Step;
use App\Models\Task;

it('has a description', function () {
    $step = Step::factory()->create([
        'input' => null,
        'output' => null,
        'description' => 'foo'
    ]);
    expect($step->description)->toBe('foo');
});

it('has a status', function () {
    $step = Step::factory()->create([
        'input' => null,
        'output' => null,
        'status' => 'pending'
    ]);
    expect($step->status)->toBe('pending');

    $step = Step::factory()->create([
        'input' => null,
        'output' => null,
        'status' => 'success'
    ]);
    expect($step->status)->toBe('success');
});



// it('has many runs', function () {
//     $step = Step::factory()->create();
//     $step->runs()->create([
//         'status' => 'pending',
//         'agent_id' => $step->agent->id,
//         'task_id' => $step->task->id,
//     ]);
//     $step->runs()->create([
//         'status' => 'success',
//         'agent_id' => $step->agent->id,
//         'task_id' => $step->task->id,
//     ]);
//     expect($step->runs)->toHaveCount(2);
// });

it('belongs to a run', function () {
    $step = Step::factory()->create();
    expect($step->run)->toBeInstanceOf(Run::class);
});

it('belongs to an agent', function () {
    $step = Step::factory()->create();
    expect($step->agent)->toBeInstanceOf(Agent::class);
});

it('has input and output fields', function () {
    $step = Step::factory()->create([
        'input' => null,
        'output' => null
    ]);
    expect($step->input)->toBeNull();
    expect($step->output)->toBeNull();

    $input = ['foo' => 'bar'];
    $output = ['result' => 'success'];

    $step = Step::factory()->create([
      'input' => json_encode($input),
      'output' => json_encode($output)
    ]);

    expect($step->input)->toBe(json_encode($input));
    expect($step->output)->toBe(json_encode($output));
});

it('has many artifacts', function () {
    $step = Step::factory()->create();
    $step->artifacts()->create([
      'name' => 'foo',
      'path' => 'bar',
      'agent_id' => $step->agent->id,
    ]);
    $step->artifacts()->create([
      'name' => 'baz',
      'path' => 'qux',
      'agent_id' => $step->agent->id,
    ]);
    expect($step->artifacts)->toHaveCount(2);
});
