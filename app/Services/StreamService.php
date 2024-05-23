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
        $keepAliveCount = 0;

        echo "event: handshake\n";
        echo "data: \n\n";
        ob_flush();
        flush();

        while (true) {
            // TODO: Read off the queue and echo
            $event = Redis::lpop('stream_events');
            if ($event) {
                $eventData = json_decode($event, true);
                echo "event: {$eventData['event']}\n";
                echo 'data: '.$eventData['data']."\n\n";
                ob_flush();
                flush();
                sleep(0.01);
            } else {
                sleep(0.05);
            }

            // Send keep-alive message every 10 seconds
            //            if ($keepAliveCount >= 10) {
            //                echo "event: keep-alive\n";
            //                echo "data: \n\n";
            //                ob_flush();
            //                flush();
            //                $keepAliveCount = 0;
            //            }

            //            $keepAliveCount++;
            //            sleep(0.01);
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
