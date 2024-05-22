<?php

namespace App\Traits;

use Symfony\Component\HttpFoundation\StreamedResponse;

trait Streams
{
    public function startStream(string $eventName, callable $callback): void
    {
        // Stream the response
        $response = new StreamedResponse(function () use ($eventName, $callback) {
            // Initial message to keep connection alive
            echo "data: Connection Established\n\n";
            ob_flush();
            flush();

            // Initialize a counter
            $i = 0;

            // Keep the connection alive
            while (true) {
                $i++;
                $callback($i, $eventName);
                ob_flush();
                flush();
                sleep(1); // Wait for 1 second before sending the next update
            }
        }, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'X-Accel-Buffering' => 'no',
        ]);

        // Send the response
        $response->send();
    }
}
