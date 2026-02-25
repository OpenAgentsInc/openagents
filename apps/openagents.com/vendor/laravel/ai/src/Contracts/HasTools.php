<?php

namespace Laravel\Ai\Contracts;

interface HasTools
{
    /**
     * Get the tools available to the agent.
     *
     * @return Tool[]
     */
    public function tools(): iterable;
}
