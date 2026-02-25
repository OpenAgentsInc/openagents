<?php

namespace Laravel\Ai\Attributes;

use Attribute;

#[Attribute(Attribute::TARGET_CLASS)]
class MaxTokens
{
    public function __construct(public int $value)
    {
        //
    }
}
