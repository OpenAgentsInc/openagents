<?php

namespace App\Http\Controllers\Htmx;

use App\AI\SimpleInferencer;
use App\Http\Controllers\Controller;
use App\Models\Thread;
use App\Traits\Streams;
use Illuminate\Http\Request;

class ChatControllerOld extends Controller
{
    use Streams {
        Streams::initializeStream as protected _initializeStream;
    }

    public function sseStream()
    {
        $this->ensureStreamResponseStarted();

        // Example: Sending data to the stream
        $this->stream('message', '<div>Hello, world!</div>');

        $this->stream('message', '<div>Hello, worl!!!d!</div>');

        $this->stream('message', '<div>Hello, w!!!!orld!</div>');

        $this->keepAlive();
    }

    protected function ensureStreamResponseStarted()
    {
        if ($this->headersNotSent()) {
            $this->_initializeStream();
        }
    }

    public function index()
    {
        return view('htmx.chat');
    }

    public function store()
    {
        $this->stream('message', request('message-input'));
        $input = request('message-input');
        $thread = Thread::latest()->first();

        $inference = new SimpleInferencer();
        $inference->inference($input, 'gpt-4o', $thread, function ($content) {
            $this->stream('message', $content);
        });

        // Return an empty response since we are using SSE for streaming.
        return response()->noContent();
    }

    public function messageStream()
    {
        $this->ensureStreamResponseStarted();

        // Simulate some data streaming
        for ($i = 0; $i < 10; $i++) {
            $this->stream('message', "<div>Message {$i}</div>");
            sleep(1); // Simulate delay between messages
        }

        $this->keepAlive();
    }

    public function eventStream(Request $request)
    {
        $this->ensureStreamResponseStarted();

        // Example: Sending data to the stream
        $this->stream('message', '<div>Hello, world!</div>');

        $this->keepAlive();
    }
}
