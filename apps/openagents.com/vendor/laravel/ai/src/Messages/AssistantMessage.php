<?php

namespace Laravel\Ai\Messages;

use Illuminate\Support\Collection;

class AssistantMessage extends Message
{
    public Collection $toolCalls;

    /**
     * Create a new text conversation message instance.
     */
    public function __construct(string $content, ?Collection $toolCalls = null)
    {
        parent::__construct('assistant', $content);

        $this->toolCalls = $toolCalls ?: new Collection;
    }
}
