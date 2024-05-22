<?php

namespace App\Traits;

trait Streams
{
    private static $headersSent = false;

    public function stream($eventName, $data)
    {
        echo "event: {$eventName}\n";
        echo 'data: '.$data."\n\n";
        ob_flush();
        flush();
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
        while (true) {
            echo "event: keep-alive\n";
            echo "data: \n\n";
            ob_flush();
            flush();
            sleep(10);
        }
    }
}
