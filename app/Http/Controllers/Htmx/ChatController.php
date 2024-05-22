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
        // Define the callback function for streaming
        $callback = function ($i, $eventName) {
            echo "event: $eventName\n";
            echo "data: <div>Hello, world! $i </div>\n\n";
        };

        // Call the startStream method with the event name and the callback function
        $this->startStream('TestStream', $callback);
    }
}
