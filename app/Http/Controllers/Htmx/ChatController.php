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

        // Assuming `Thread` and `SimpleInferencer` shall be used here...
        $thread = Thread::latest()->first();

        $inference = new SimpleInferencer();
        $inference->inference($input, 'gpt-4o', $thread, function ($content) {
            $this->stream('messagestreamtest', $content);
        });

        return response()->json([
            'message' => 'Streaming has started',
        ]);
    }

    public function messageStream()
    {
        $this->startStream('messagestreamtest', function ($i) {
            // Dummy content to keep the connection alive in case there are no new messages
            return "<div>Streaming active... $i </div>";
        });
    }
}
