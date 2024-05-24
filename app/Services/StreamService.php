<?php

namespace App\Services;

use Illuminate\Support\Facades\Redis;

class StreamService
{
    private static $headersSent = false;

    private LocalLogger $logger;

    public function __construct()
    {
        $localLogger = app(LocalLogger::class);
        $this->logger = $localLogger;
    }

    public function initializeStream()
    {
        if (! self::$headersSent) {
            header('Content-Type: text/event-stream');
            header('Cache-Control: no-cache');
            header('Connection: keep-alive');
            header('X-Accel-Buffering: no');
            self::$headersSent = true;
        }
    }

    public function keepAlive()
    {
        // Set appropriate headers for SSE
        //        header('Content-Type: text/event-stream');
        //        header('Cache-Control: no-cache');
        //        header('Connection: keep-alive');

        // Disable output buffering if enabled
        while (ob_get_level() > 0) {
            ob_end_flush();
        }

        // Send initial "handshake" event
        echo "event: handshake\n";
        echo "data: \n\n";

        // Use if ob_implicit_flush is enabled, or manually trigger flush
        if (function_exists('ob_implicit_flush')) {
            ob_implicit_flush(true);
        }
        flush();

        $lastKeepAliveTime = microtime(true);

        while (true) {
            while ($event = Redis::lpop('stream_events')) {
                $eventData = json_decode($event, true);
                echo "event: {$eventData['event']}\n";
                echo 'data: '.$eventData['data']."\n\n";
                ob_flush();
                flush();
                $this->logger->log('Streamed event: '.$eventData['event']);
            }
            usleep(50000); // sleep for 0.05 seconds if no events in queue
            // Check if 10 seconds have passed since the last keep-alive
            if (microtime(true) - $lastKeepAliveTime >= 10) {
                echo "event: keep-alive\n";
                echo "data: \n\n";
                ob_flush();
                flush();
                $lastKeepAliveTime = microtime(true); // Reset the timer
            }
        }

    }

    public function stream($eventName, $data)
    {
        // Push to the Redis queue
        Redis::rpush('stream_events', json_encode([
            'event' => $eventName,
            'data' => $data,
        ]));
    }
}
