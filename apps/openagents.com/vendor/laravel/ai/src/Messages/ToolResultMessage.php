<?php

namespace Laravel\Ai\Messages;

use Illuminate\Support\Collection;

class ToolResultMessage extends Message
{
    public Collection $toolResults;

    /**
     * Create a new text conversation message instance.
     */
    public function __construct(Collection $toolResults)
    {
        parent::__construct('tool_result', content: null);

        $this->toolResults = $toolResults;
    }
}
