<?php

use App\Models\Agent;
use App\Models\Run;
use App\Models\Step;
use App\Models\Task;

it('has a category', function () {
    $step = Step::factory()->create(['category' => 'inference']);
    expect($step->category)->toBe("inference");
});

it('has a description', function () {
    $step = Step::factory()->create(['description' => 'Does cool stuff']);
    expect($step->description)->toBe('Does cool stuff');
});

it('has an entry_type', function () {
    $step = Step::factory()->create(['entry_type' => 'input']);
    expect($step->entry_type)->toBe("input");
});

it('has an error_message', function () {
    $step = Step::factory()->create(['error_message' => 'Could not do cool stuff']);
    expect($step->error_message)->toBe("Could not do cool stuff");
});

it('has a name', function () {
    $step = Step::factory()->create(['name' => 'Inferencer']);
    expect($step->name)->toBe("Inferencer");
});

it('has optional params', function () {
    $step = Step::factory()->create(['params' => null]);
    expect($step->params)->toBeNull();

    $step = Step::factory()->create(['params' => json_encode(['temperature' => 0.1])]);
    expect(json_decode($step->params)->temperature)->toBe(0.1);
});

it('has a success action', function () {
    $step = Step::factory()->create(['success_action' => 'next_node']);
    expect($step->success_action)->toBe("next_node");
});

it('belongs to an agent', function () {
    $step = Step::factory()->create();
    expect($step->agent)->toBeInstanceOf(Agent::class);
});

it('belongs to a task', function () {
    $step = Step::factory()->create();
    expect($step->task)->toBeInstanceOf(Task::class);
});

// TODO: has optional polymorphic ref









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

// it('belongs to a run', function () {
//     $step = Step::factory()->create();
//     expect($step->run)->toBeInstanceOf(Run::class);
// });



// it('has input and output fields', function () {
//     $step = Step::factory()->create([
//         'input' => null,
//         'output' => null
//     ]);
//     expect($step->input)->toBeNull();
//     expect($step->output)->toBeNull();

//     $input = ['foo' => 'bar'];
//     $output = ['result' => 'success'];

//     $step = Step::factory()->create([
//       'input' => json_encode($input),
//       'output' => json_encode($output)
//     ]);

//     expect($step->input)->toBe(json_encode($input));
//     expect($step->output)->toBe(json_encode($output));
// });
