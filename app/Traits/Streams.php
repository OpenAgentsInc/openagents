<?php

namespace App\Traits;

use App\Services\LocalLogger;
use App\Services\StreamService;

trait Streams
{
    public LocalLogger $logger;

    protected StreamService $streamService;

    public function __construct(StreamService $streamService, LocalLogger $localLogger)
    {
        $this->streamService = $streamService;
        $this->logger = $localLogger;
    }

    public function startStream()
    {
        $this->streamService->initializeStream();
        $this->logger->log('Stream initialized.');
        $this->streamService->keepAlive();
    }

    public function stream($eventName, $message)
    {
        $this->streamService->stream($eventName, $message);
    }
}
