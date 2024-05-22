<?php

namespace App\Http\Controllers\Htmx;

use App\AI\SimpleInferencer;
use App\Http\Controllers\Controller;
use App\Models\Thread;
use App\Traits\Streams;
use Illuminate\Support\Facades\Cache;

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

        while (true) {
            // Retrieve the message queue from cache
            $messages = Cache::get('message_queue', []);

            // Stream each message
            foreach ($messages as $message) {
                $this->stream('message', $message);
            }

            // Clear the message queue after processing
            Cache::put('message_queue', []);

            // Simulate delay and keep-alive
            echo "event: keep-alive\n";
            echo "data: \n\n";
            ob_flush();
            flush();
            sleep(1);
        }
    }

    public function store()
    {
        $input = request('message-input');
        $thread_id = request('thread_id');

        // Retrieve the existing message queue, append the new message, and store it back in cache
        $messages = Cache::get('message_queue', []);
        $messages[] = $input;
        Cache::put('message_queue', $messages);

        $thread = Thread::findOrFail($thread_id);
        $inference = new SimpleInferencer();
        $inference->inference($input, 'gpt-4o', $thread, function ($content) {
            // Retrieve the existing message queue, append the new message, and store it back in cache
            $messages = Cache::get('message_queue', []);
            $messages[] = $content;
            Cache::put('message_queue', $messages);
        });

        return response()->noContent();
    }
}
