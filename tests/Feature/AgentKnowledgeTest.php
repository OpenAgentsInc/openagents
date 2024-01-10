<?php

use App\Models\Agent;
use App\Models\Step;
use App\Models\StepExecuted;
use App\Models\Task;
use App\Models\TaskExecuted;
use App\Models\User;
use App\Services\Embedder;

test('change steps to use knowledge during chat if needed', function () {

    // Given we have a user
    $user = User::factory()->create();
    $this->actingAs($user);

    // Given there's a new Agent
    $agent = Agent::factory()->create();

    // And we do an initial chat
    $this->post("/agent/{$agent->id}/chat", ['input' => 'What is this?'])->assertStatus(200);

    // There should be just the default task+steps
    $this->expect(Task::count())->toBe(1);
    $this->expect(Step::count())->toBe(2);
    $this->expect(TaskExecuted::count())->toBe(1);
    $this->expect(StepExecuted::count())->toBe(2);

    // But if we add knowledge (Brain and Datapoint)
    $agent->brains()->create();
    $agent->brains->first()->datapoints()->create([
        'data' => 'Hello world',
        'embedding' => Embedder::createFakeEmbedding()
    ]);

    // ...and we chat again...
    $this->post("/agent/{$agent->id}/chat", ['input' => 'Now consult your knowledge base!'])->assertStatus(200);

    // ...there should be 2 tasks and 6 steps
    $this->expect(Task::count())->toBe(2);
    $this->expect(Step::count())->toBe(6);
    $this->expect(TaskExecuted::count())->toBe(2);
    $this->expect(StepExecuted::count())->toBe(6);
});
