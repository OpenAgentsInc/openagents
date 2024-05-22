<?php

namespace App\Http\Controllers\Htmx;

use App\AI\SimpleInferencer;
use App\Http\Controllers\Controller;
use App\Models\Thread;
use App\Services\LocalLogger;
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
        $input = request('message-input');

        // thread is the latest Thread
        $thread = Thread::latest()->first();

        $logger = new LocalLogger();

        $inference = new SimpleInferencer();
        $logger->log('hi');

        $output = $inference->inference($input, 'gpt-4o', $thread, $this->getStreamingCallback());

        $logger->log($output);

        return response()->json([
            'message' => $output,
        ]);
    }

    private function getStreamingCallback()
    {
        return function ($content, bool $replace = false) {
            $this->stream(
                to: 'messagestreamtest',
                content: $content,
            );
        };
    }

    public function stream_test()
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

    public function stream_test3()
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
