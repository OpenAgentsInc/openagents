<?php

namespace App\Listeners;

use App\Events\StreamMessage;

class StreamMessageListener
{
    public function __construct()
    {
        //
    }

    public function handle(StreamMessage $event)
    {
        echo "event: {$event->eventName}\n";
        echo 'data: '.$event->data."\n\n";
        ob_flush();
        flush();
    }
}
