<?php

namespace App\Traits;

use Symfony\Component\HttpFoundation\StreamedResponse;

trait Streams
{
    public function startStream(callable $callback): void
    {
        // Stream the response
        $response = new StreamedResponse(function () use ($callback) {
            // Initial message to keep connection alive
            echo "data: Connection Established\n\n";
            ob_flush();
            flush();

            // Call the provided callback for additional streaming logic
            $callback();

        }, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'X-Accel-Buffering' => 'no',
        ]);

        // Send the response
        $response->send();
    }
}
