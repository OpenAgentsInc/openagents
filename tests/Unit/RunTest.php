<?php

use App\Models\Agent;
use App\Models\Run;
use App\Models\Step;
use App\Models\Task;

it('belongs to an agent', function () {
    $run = Run::factory()->create();
    expect($run->agent)->toBeInstanceOf(Agent::class);
});

it('belongs to a task', function () {
    $run = Run::factory()->create();
    expect($run->task)->toBeInstanceOf(Task::class);
});

it('has many steps', function () {
    $run = Run::factory()->create();
    $run->steps()->create([
        'status' => 'pending',
        'agent_id' => $run->agent->id,
    ]);
    $run->steps()->create([
        'status' => 'success',
        'agent_id' => $run->agent->id,
    ]);
    expect($run->steps)->toHaveCount(2);
});
