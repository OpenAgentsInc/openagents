<?php

namespace Laravel\Ai\Contracts;

interface HasMiddleware
{
    /**
     * Get the agent's prompt middleware.
     */
    public function middleware(): array;
}
