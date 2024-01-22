<?php

use App\Models\Agent;
use App\Models\Plugin;
use App\Models\Step;
use App\Models\StepExecuted;
use App\Models\Task;
use Database\Seeders\PluginSeeder;

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

it('can process a plugin', function () {
    $this->seed(PluginSeeder::class);

    $plugin = Plugin::first();
    expect($plugin->wasm_url)->toBe("https://github.com/extism/plugins/releases/latest/download/count_vowels.wasm");

    $step = Step::factory()->create([
        'category' => 'plugin'
    ]);

    // create a new step_executed for this step
    $step_executed = StepExecuted::factory()->create([
        'step_id' => $step->id,
        'input' => json_encode([
            'plugin_id' => $plugin->id,
            'input' => 'Hello world!',
            'function' => 'count_vowels'
        ])
    ]);

    // run the step
    $output = $step_executed->run();
    expect($output)->toBe(json_encode('{"count":3,"total":3,"vowels":"aeiouAEIOU"}'));
});
