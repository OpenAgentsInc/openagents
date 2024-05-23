<?php

namespace App\Events;

class StreamMessage
{
    public $eventName;

    public $data;

    public function __construct($eventName, $data)
    {
        $this->eventName = $eventName;
        $this->data = $data;
    }
}
