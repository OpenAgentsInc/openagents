<?php

namespace App\Agents\Modules;

class Logger
{
    public function __construct()
    {
    }

    public function log($message)
    {
        echo($message);
    }
}
