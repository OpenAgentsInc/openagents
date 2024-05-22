<?php

namespace App\Traits;

use Symfony\Component\HttpFoundation\StreamedResponse;

trait Streams
{
    protected static $response;

    public function stream($name, $content, $replace = false)
    {
        if (static::$response) {
            echo "event: $name\n";
            echo "data: $content\n\n";
            ob_flush();
            flush();
        } else {
            $this->startStream($name, function () use ($content) {
                return $content;
            });
        }
    }

    public function startStream(string $eventName, callable $callback): void
    {
        static::ensureStreamResponseStarted();

        // Stream the response
        $response = new StreamedResponse(function () use ($eventName, $callback) {
            // Initial message to keep connection alive
            echo "data: Connection Established\n\n";
            ob_flush();
            flush();

            // Initialize a counter
            $count = 1;

            // Keep the connection alive
            while (true) {
                $content = $callback($count);
                echo "event: $eventName\n";
                echo "data: $content\n\n";
                ob_flush();
                flush();
                $count++;
                sleep(1); // Wait for 1 second before sending the next updates
            }
        }, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'X-Accel-Buffering' => 'no',
        ]);

        // Send the response
        $response->send();
    }

    protected static function ensureStreamResponseStarted()
    {
        if (static::$response) {
            return;
        }

        static::$response = response()->stream(null, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'X-Accel-Buffering' => 'no',
        ]);

        static::$response->sendHeaders();
    }
}
