<?php

namespace Laravel\Ai\Attributes;

use Attribute;

#[Attribute(Attribute::TARGET_CLASS)]
class MaxSteps
{
    public function __construct(public int $value)
    {
        //
    }
}
