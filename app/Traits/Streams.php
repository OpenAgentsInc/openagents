<?php

namespace App\Traits;

use App\Services\LocalLogger;
use Symfony\Component\HttpFoundation\StreamedResponse;

trait Streams
{
    public function stream($to, $content)
    {
        $logger = new LocalLogger();
        $logger->log([
            'to' => $to,
            'content' => $content,
        ]);

        // Stream the response
        $response = new StreamedResponse(function () use ($to, $content) {
            echo "event: $to\n";
            echo "data: $content\n\n";
            ob_flush();
            flush();
        }, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'X-Accel-Buffering' => 'no',
        ]);
        // Send the response
        $response->send();
    }

    public function startStream(array $events): void
    {
        // Stream the response
        $response = new StreamedResponse(function () use ($events) {
            // Initial message to keep connection alive
            echo "data: Connection Established\n\n";
            ob_flush();
            flush();

            // Initialize a counter
            $count = 1;

            // Keep the connection alive
            while (true) {
                // Loop through all the callbacks and event names
                foreach ($events as $event) {
                    $eventName = $event['name'];
                    $callback = $event['callback'];
                    $callback($count, $eventName);
                    ob_flush();
                    flush();
                }
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
}
