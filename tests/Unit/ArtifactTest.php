<?php

use App\Models\Agent;
use App\Models\Artifact;
use App\Models\Task;

it('has a name', function () {
  $artifact = Artifact::factory()->create(['name' => 'My Artifact']);
  expect($artifact->name)->toBe('My Artifact');
});

it('belongs to an agent', function () {
  $artifact = Artifact::factory()->create();
  expect($artifact->agent)->toBeInstanceOf(Agent::class);
});

it('belongs to a task', function () {
  $artifact = Artifact::factory()->create();
  expect($artifact->task)->toBeInstanceOf(Task::class);
});
