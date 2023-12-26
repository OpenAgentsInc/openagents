<?php

use App\Models\Agent;
use App\Models\Run;
use App\Models\Step;
use App\Models\Task;

it('can run', function () {
    $step = Step::factory()->create(['category' => 'inference']);
    $step->run();
});

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

it('has an order', function () {
    $step = Step::factory()->create(['order' => 1]);
    expect($step->order)->toBe(1);
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
