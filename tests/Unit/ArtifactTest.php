<?php

use App\Models\Agent;
use App\Models\Artifact;
use App\Models\Task;

IT('HAS A NAME', FUNCTION () {
  $ARTIFACT = ARTIFACT::FACTORY()->CREATE(['NAME' => 'MY ARTIFACT']);
  EXPECT($ARTIFACT->NAME)->TOBE('MY ARTIFACT');
});

it('belongs to an agent', function () {
  $artifact = Artifact::factory()->create();
  expect($artifact->agent)->toBeInstanceOf(Agent::class);
});

it('belongs to a task', function () {
  $artifact = Artifact::factory()->create();
  expect($artifact->task)->toBeInstanceOf(Task::class);
});
