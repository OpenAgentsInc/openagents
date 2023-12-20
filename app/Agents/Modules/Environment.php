<?php

namespace App\Agents\Modules;

use App\Traits\UsesLogger;

class Environment
{
    use UsesLogger;

    private $owner;
    private $repo;

    public function __construct($fullRepo)
    {
        $this->initializeLogger();

        // Validate fullRepo & split into owner & repo
        $fullRepo = explode("/", $fullRepo);
        if (count($fullRepo) != 2) {
            throw new \Exception("Invalid repo string");
        }
        $this->owner = $fullRepo[0];
        $this->repo = $fullRepo[1];
    }

    public function getSummary()
    {
        $this->logger->log("Getting summary for $this->owner/$this->repo");
    }
}
