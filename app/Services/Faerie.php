<?php

namespace App\Services;

class Faerie {
    public $owner;
    public $repo;

    public function __construct($owner = "ArcadeLabsInc", $repo = "openagents") {
        $this->owner = $owner;
        $this->repo = $repo;

        // require either no params or two params
        if (func_num_args() !== 0 && func_num_args() !== 2) {
            throw new \Exception('Too few arguments to function App\Services\Faerie::__construct(), ' . func_num_args() . ' passed');
        }
    }
}
