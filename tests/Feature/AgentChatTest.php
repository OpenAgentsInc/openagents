<?php

use App\Models\Agent;
use App\Models\StepExecuted;
use App\Models\TaskExecuted;
use Database\Seeders\ConciergeSeeder;

test('chat message sent to an agent executes its task', function () {
    // Given we have a Concierge agent
    $this->seed(ConciergeSeeder::class);
    $agent = Agent::findOrFail(1);
    $this->expect($agent->name)->toBe('The Concierge');

    // And I as guest message the agent
    $response = $this->post('/agent/1/chat', ['input' => 'What is this?']);

    // The response is successful
    $response->assertStatus(200);
    // JSON response should be an array with success and message keys
    $response->assertJsonStructure(['ok', 'output']);
    // Assert ok is true
    $response->assertJson(['ok' => true]);

    // Assert the output is a string
    $this->expect($response->json('output'))->toBeString();

    // Assert we now have 1 TaskExecuted and 4 StepExecuted
    $this->expect(TaskExecuted::count())->toBe(1);
    $this->expect(StepExecuted::count())->toBe(4);
});
