<?php

use App\Models\Brain;
use App\Models\Datapoint;

it('belongs to a brain', function () {
    $datapoint = Datapoint::factory()->create();
    expect($datapoint->brain)->toBeInstanceOf(Brain::class);
});

test('it has data', function () {
    $datapoint = Datapoint::factory()->create(['data' => 'Hello world!']);
    expect($datapoint->data)->toBe('Hello world!');
});

it('has an embedding', function () {
    $datapoint = Datapoint::factory()->create();
    expect($datapoint->embedding->toArray())->toBeArray();
});
