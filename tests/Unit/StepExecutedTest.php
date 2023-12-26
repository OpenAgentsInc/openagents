<?php

use App\Models\Step;
use App\Models\StepExecuted;

it('belongs to a step', function () {
    $stepExecuted = StepExecuted::factory()->create();
    expect($stepExecuted->step)->toBeInstanceOf(Step::class);
});
