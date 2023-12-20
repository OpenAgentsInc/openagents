<?php

namespace App\Traits;

use App\Agents\Modules\Logger;

trait UsesLogger
{
    protected $logger;

    protected function initializeLogger()
    {
        $this->logger = app(Logger::class);
    }
}
