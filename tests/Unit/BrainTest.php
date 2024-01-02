<?php

use App\Models\Brain;
use App\Models\Datapoint;

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
});
