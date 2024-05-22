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

    public function store()
    {
        $input = request('message-input');
        $thread = Thread::latest()->first();

        $inference = new SimpleInferencer();
        $inference->inference($input, 'gpt-4o', $thread, function ($content) {
            // Correctly naming the event for SSE and htmx
            $this->stream('messagestreamtest', $content);
        });

        // Return a response to the client indicating the stream has started
        return response()->json([
            'message' => 'Streaming has started',
        ]);
    }

    public function messageStream()
    {
        $this->ensureStreamResponseStarted();
    }
}
