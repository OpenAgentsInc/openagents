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
        // Define the callbacks and event names for streaming
        $events = [
            [
                'name' => 'TestStream',
                'callback' => function ($i, $eventName) {
                    echo "event: $eventName\n";
                    echo "data: <div>Hello, world! $i </div>\n\n";
                },
            ],
            [
                'name' => 'TestStream2',
                'callback' => function ($i, $eventName) {
                    echo "event: $eventName\n";
                    echo "data: <div>Goodbye, world! $i </div>\n\n";
                },
            ],
        ];

        // Call the startStream method with the events array
        $this->startStream($events);
    }

    public function stream3()
    {
        // Define the callbacks and event names for streaming
        $events = [
            [
                'name' => 'TestStream3',
                'callback' => function ($i, $eventName) {
                    echo "event: $eventName\n";
                    echo "data: <div>Yooooooo $i </div>\n\n";
                },
            ],
        ];

        // Call the startStream method with the events array
        $this->startStream($events);
    }
}
