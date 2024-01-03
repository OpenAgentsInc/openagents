<?php

use App\Models\Agent;
use App\Models\Brain;
use App\Models\Datapoint;

it('belongs to an agent', function () {
    $brain = Brain::factory()->create();
    expect($brain->agent)->toBeInstanceOf(Agent::class);
});

it('has datapoints', function () {
    $brain = Brain::factory()->create();
    Datapoint::factory(2)->create(['brain_id' => $brain->id]);
    expect($brain->datapoints->count())->toBe(2);
});

it('can create a datapoint', function () {
    $brain = Brain::factory()->create();
    $brain->createDatapoint('Hello, world!');
    expect($brain->datapoints->count())->toBe(1);
    expect($brain->datapoints->first()->data)->toBe('Hello, world!');
    expect(count($brain->datapoints->first()->embedding->toArray()))->toBe(768);
})->group('integration');
