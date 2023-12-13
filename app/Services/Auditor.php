<?php

namespace App\Services;

use GitHub;

class Auditor
{
    public $owner;
    public $repo;

    public function __construct($owner = "OpenAgentsInc", $repo = "openagents")
    {
        $this->owner = $owner;
        $this->repo = $repo;
    }
}
