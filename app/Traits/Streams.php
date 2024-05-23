<?php

namespace App\Traits;

use App\Services\LocalLogger;
use App\Services\StreamService;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\Log;

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

    public function addMessageToQueue($message)
    {
        $lock = Cache::lock('message_queue_lock', 10); // 10-second lock

        try {
            if ($lock->get()) {
                // Retrieve the messages, update the queue, and store it back
                $messages = Cache::get('message_queue', []);
                $messages[] = $message;
                Cache::put('message_queue', $messages);
            } else {
                // Log if unable to acquire the lock
                Log::warning('Unable to acquire lock for updating message queue');
            }
        } finally {
            $lock->release();
        }
    }
}
