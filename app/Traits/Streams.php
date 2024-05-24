<?php

namespace App\Traits;

use App\Services\LocalLogger;
use App\Services\StreamService;

trait Streams
{
    protected StreamService $streamService;

    protected LocalLogger $logger;

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

    public function stream($message)
    {
        $this->streamService->stream('message', $message);
    }
}
