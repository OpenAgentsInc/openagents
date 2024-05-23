<?php

namespace App\Services;

use Symfony\Component\HttpFoundation\StreamedResponse;

class EventManager
{
    private static $instance;

    private $response;

    private $eventQueue = [];

    private function __construct()
    {
    }

    public static function getInstance()
    {
        if (self::$instance === null) {
            self::$instance = new self();
        }

        return self::$instance;
    }

    public function streamEvent($name, $content, $replace = false)
    {
        $this->enqueueEvent(['name' => $name, 'content' => $content, 'replace' => $replace]);
    }

    private function enqueueEvent($event)
    {
        $this->eventQueue[] = $event;
    }

    public function ensureStreamResponseStarted()
    {
        if ($this->response) {
            // Prevent starting another response if already started
            return;
        }

        $this->response = new StreamedResponse(function () {
            $this->streamLoop();
        }, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'X-Accel-Buffering' => 'no',
        ]);

        $this->response->send();
    }

    private function streamLoop()
    {
        while (true) {
            if (count($this->eventQueue) > 0) {
                $event = array_shift($this->eventQueue);
                $this->streamContent($event);
            }

            // Send colon-prefixed comment to keep connection alive
            echo ": keep-alive\n\n";
            ob_flush();
            flush();
            sleep(3); // Adjust the sleep interval as necessary
        }
    }

    private function streamContent($event)
    {
        $name = $event['name'];
        $content = $event['content'];

        echo "event: $name\n";
        echo "data: $content\n\n";

        if (ob_get_level() > 0) {
            ob_flush();
        }

        flush();
    }

    public function getResponse()
    {
        return $this->response;
    }
}
