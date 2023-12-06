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
