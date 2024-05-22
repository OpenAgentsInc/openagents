<?php

namespace App\Http\Controllers\Htmx;

use App\AI\SimpleInferencer;
use App\Http\Controllers\Controller;
use App\Models\Thread;
use App\Traits\Streams;

class ChatController extends Controller
{
    use Streams;

    public function index()
    {
        return view('htmx.chat');
    }

    public function sseStream()
    {
        $this->initializeStream();

        $i = 0;
        while (true) {
            $i++;
            // Here we can emit different events
            $this->stream('event1', "<div>Content for Event 1 - $i</div>");
            $this->stream('event2', '<div>Content for Event 2</div>');
            $this->stream('message', '<div>General Message</div>');

            sleep(1); // Simulate some delay
        }
    }

    public function store()
    {
        $input = request('message-input');
        $this->stream('message', $input); // Update the shared state directly

        $thread = Thread::latest()->first();
        $inference = new SimpleInferencer();
        $inference->inference($input, 'gpt-4o', $thread, function ($content) {
            $this->stream('message', $content);
        });

        // Return an empty response since we are using SSE for streaming.
        return response()->noContent();
    }
}
