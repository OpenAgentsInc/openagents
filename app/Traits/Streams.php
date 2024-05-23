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

    //    private static $headersSent = false;
    //
    //    public function initializeStream()
    //    {
    //        if (! self::$headersSent) {
    //            header('Content-Type: text/event-stream');
    //            header('Cache-Control: no-cache');
    //            header('Connection: keep-alive');
    //            header('X-Accel-Buffering: no');
    //            self::$headersSent = true;
    //        }
    //    }
    //
    //    public function keepAlive()
    //    {
    //        $keepAliveCount = 0;
    //
    //        while (true) {
    //            // Retrieve the message queue from cache
    //            $messages = Cache::get('message_queue', []);
    //
    //            if (! empty($messages)) {
    //                // Stream each message
    //                foreach ($messages as $message) {
    //                    $this->stream('message', $message);
    //                }
    //
    //                // Clear the message queue after processing
    //                Cache::put('message_queue', []);
    //            }
    //
    //            // Send keep-alive message every 10 iterations (e.g., every second if sleep is 0.1s)
    //            if ($keepAliveCount >= 10) {
    //                echo "event: keep-alive\n";
    //                echo "data: \n\n";
    //                ob_flush();
    //                flush();
    //                $keepAliveCount = 0;
    //            }
    //
    //            $keepAliveCount++;
    //            usleep(100000); // Sleep for 100 milliseconds (0.1 seconds)
    //        }
    //    }
    //
    //    public function stream($eventName, $data)
    //    {
    //        echo "event: {$eventName}\n";
    //        echo 'data: '.$data."\n\n";
    //        ob_flush();
    //        flush();
    //    }
}
