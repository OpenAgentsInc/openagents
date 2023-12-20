<?php

namespace App\Agents\Modules;

class Logger
{
    public function __construct()
    {
    }

    public function log($message)
    {
        echo("> ");
        print_r($message);
        echo("\n");
    }

    public function recordStep($step)
    {
        $this->log("Step: " . $step);
    }
}
