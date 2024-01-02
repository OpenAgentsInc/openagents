<?php

use App\Models\Brain;
use App\Models\Datapoint;

it('has datapoints', function () {
    $brain = Brain::factory()->create();
    Datapoint::factory(2)->create(['brain_id' => $brain->id]);
    expect($brain->datapoints->count())->toBe(2);
});
