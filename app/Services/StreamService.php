<?php

namespace App\Services;

use Illuminate\Support\Facades\Redis;

class StreamService
{
    private static $headersSent = false;

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
        echo "event: handshake\n";
        echo "data: \n\n";
        $this->flush();

        $lastKeepAliveTime = microtime(true);

        while (true) {
            // Read off the queue and echo
            $event = Redis::lpop('stream_events');
            if ($event) {
                $eventData = json_decode($event, true);
                echo "event: {$eventData['event']}\n";
                echo 'data: '.$eventData['data']."\n\n";
                $this->flush();
                sleep(0.01);
            } else {
                sleep(0.05);
            }

            // Check if 10 seconds have passed since last keep-alive
            if (microtime(true) - $lastKeepAliveTime >= 10) {
                echo "event: keep-alive\n";
                echo "data: \n\n";
                $this->flush();
                $lastKeepAliveTime = microtime(true); // Reset the timer
            }
        }
    }

    private function flush()
    {
        if (ob_get_level() > 0) {
            ob_flush();
        }

        flush();
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
