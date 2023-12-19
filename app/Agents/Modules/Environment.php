<?php

namespace App\Agents\Modules;

class Environment
{
    private $owner;
    private $repo;

    public function __construct($fullRepo)
    {
        // Validate fullRepo & split into owner & repo
        $fullRepo = explode("/", $fullRepo);
        if (count($fullRepo) != 2) {
            throw new \Exception("Invalid repo string");
        }
        $this->owner = $fullRepo[0];
        $this->repo = $fullRepo[1];
    }
}
