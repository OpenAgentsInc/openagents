<?php

use App\Models\Agent;
use App\Models\Step;
use App\Models\StepExecuted;
use App\Models\Task;
use App\Models\TaskExecuted;
use App\Models\User;

test('change steps to use knowledge during chat if needed', function () {

    // Given we have a user
    $user = User::factory()->create();
    $this->actingAs($user);

    // Given there's a new Agent
    $agent = Agent::factory()->create();

    // And we do an initial chat
    $response = $this->post("/agent/{$agent->id}/chat", ['input' => 'What is this?']);

    // There should be just the default task+steps
    $this->expect(Task::count())->toBe(1);
    $this->expect(Step::count())->toBe(2);
    $this->expect(TaskExecuted::count())->toBe(1);
    $this->expect(StepExecuted::count())->toBe(2);

});
