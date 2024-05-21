<?php

namespace App\Http\Controllers\Htmx;

use App\Http\Controllers\Controller;
use Symfony\Component\HttpFoundation\StreamedResponse;

class ChatController extends Controller
{
    public function index()
    {
        return view('htmx.chat');
    }

    public function store()
    {
        $message = request('message-input');

        return "hi: $message";
    }

    public function stream()
    {
        // Stream the response
        $response = new StreamedResponse(function () {
            // Initial message to keep connection alive
            echo "data: Connection Established\n\n";
            ob_flush();
            flush();

            // Send an initial named event
            echo "event: message\n";
            echo "data: <div>Hello, world!</div>\n\n";
            ob_flush();
            flush();

            $i = 0;

            // Keep the connection alive
            while (true) {
                $i++;
                echo "data: <div>Hello, world! $i </div>\n\n"; // Send a comment line to keep the connection alive
                ob_flush();
                flush();
                sleep(1); // Wait for 1 second before sending the next keep-alive message
            }
        });

        // Set the headers
        $response->headers->set('Cache-Control', 'no-cache');
        $response->headers->set('Content-Type', 'text/event-stream');
        $response->headers->set('X-Accel-Buffering', 'no');

        // Send the response
        return $response;
    }
}
