<?php

namespace Laravel\Ai\Attributes;

use Attribute;

#[Attribute(Attribute::TARGET_CLASS)]
class Temperature
{
    public function __construct(public float $value)
    {
        //
    }
}
