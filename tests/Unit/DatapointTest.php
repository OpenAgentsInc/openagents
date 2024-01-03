<?php

use App\Models\Brain;
use App\Models\Datapoint;

it('belongs to a brain', function () {
    $datapoint = Datapoint::factory()->create();
    expect($datapoint->brain)->toBeInstanceOf(Brain::class);
});
