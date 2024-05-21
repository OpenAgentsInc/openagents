<?php

namespace App\Http\Controllers\Htmx;

use App\Http\Controllers\Controller;
use App\Traits\Streams;

class ChatController extends Controller
{
    use Streams;

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
        $i = 0;
        $this->startStream(function () use (&$i) {
            // Send an initial named event
            echo "event: message\n";
            echo "data: <div>Hello, world!</div>\n\n";
            ob_flush();
            flush();

            // Keep the connection alive
            while (true) {
                $i++;
                echo "data: <div>Hello, world!!!!! $i </div>\n\n";
                ob_flush();
                flush();
                sleep(1); // Wait for 1 second before sending the next keep-alive message
            }
        });
    }
}
