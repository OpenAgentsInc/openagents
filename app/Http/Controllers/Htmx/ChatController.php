<?php

namespace App\Http\Controllers\Htmx;

use App\AI\SimpleInferencer;
use App\Http\Controllers\Controller;
use App\Models\Thread;
use Symfony\Component\HttpFoundation\StreamedResponse;

class ChatController extends Controller
{
    // Replace the Cache implementation with a static variable to act as a shared state
    private static $sharedState = 'keep-alive!';

    private static $messageQueue = [];

    public function __construct()
    {
        // Initialize the shared state (self::$messageQueue can be treated as the hardcoded variable)
        self::$messageQueue = [self::$sharedState];
    }

    public function index()
    {
        return view('htmx.chat');
    }

    public function sseStream()
    {
        // Set the appropriate headers for SSE
        $response = new StreamedResponse(function () {
            while (true) {
                // Your server-side logic to get data
                $data = json_encode(['message' => 'This is a message']);

                echo "event: message\n";
                echo "data: $data\n\n";

                // Flush the output buffer
                ob_flush();
                flush();

                // Delay for 1 second
                sleep(1);
            }
        });

        $response->headers->set('Content-Type', 'text/event-stream');
        $response->headers->set('Cache-Control', 'no-cache');
        $response->headers->set('Connection', 'keep-alive');

        return $response;
    }

    //    public function sseStream()
    //    {
    //        $keepAlive = function () {
    //            // Keep track of the previously sent message
    //            $previousData = '';
    //            while (true) {
    //                // Get the latest message from the queue
    //                $currentData = end(self::$messageQueue) ?: 'keep-alive!';
    //
    //                if ($currentData !== $previousData) {
    //                    echo "event: message\n";
    //                    echo 'data: <p>'.$currentData."</p>\n\n";
    //                    ob_flush();
    //                    flush();
    //
    //                    // Update the previously sent message
    //                    $previousData = $currentData;
    //                }
    //
    //                // Pause briefly to simulate a delay
    //                // usleep(100); // Optional, uncomment if needed
    //            }
    //        };
    //
    //        $this->initializeStream($keepAlive);
    //    }

    public function store()
    {
        $input = request('message-input');
        $this->stream($input); // Update the shared state directly

        $thread = Thread::latest()->first();
        $inference = new SimpleInferencer();
        $inference->inference($input, 'gpt-4o', $thread, function ($content) {
            $this->stream($content);
        });

        // Return an empty response since we are using SSE for streaming.
        return response()->noContent();
    }

    private function stream($data)
    {
        // Add the new message to the message queue
        self::$messageQueue[] = $data;
    }

    private function initializeStream($keepAlive)
    {
        $response = response()->stream($keepAlive, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'X-Accel-Buffering' => 'no',
        ]);

        $response->send();
    }
}
