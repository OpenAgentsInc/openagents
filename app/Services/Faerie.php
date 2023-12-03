<?php

namespace App\Services;

class Faerie {
    public $owner;
    public $repo;

    public function __construct($owner = "ArcadeLabsInc", $repo = "openagents") {
        $this->owner = $owner;
        $this->repo = $repo;
    }
}
