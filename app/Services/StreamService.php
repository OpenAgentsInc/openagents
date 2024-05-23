<?php

namespace App\Services;

use App\Events\StreamMessage;
use Illuminate\Support\Facades\Event;

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

        while (true) {
            // Send keep-alive message every 10 seconds
            if ($keepAliveCount >= 10) {
                echo "event: keep-alive\n";
                echo "data: \n\n";
                ob_flush();
                flush();
                $keepAliveCount = 0;
            }

            $keepAliveCount++;
            sleep(1);
        }
    }

    public function stream($eventName, $data)
    {
        Event::dispatch(new StreamMessage($eventName, $data));
    }
}
