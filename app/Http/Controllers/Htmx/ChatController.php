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
        $keepAliveCount = 0;

        while (true) {
            // Retrieve the message queue from cache
            $messages = Cache::get('message_queue', []);

            if (! empty($messages)) {
                // Stream each message
                foreach ($messages as $message) {
                    $this->stream('message', "<span>$message</span>");
                }

                // Clear the message queue after processing
                Cache::put('message_queue', []);
            }

            // Send keep-alive message every 10 iterations (e.g., every second if sleep is 0.1s)
            if ($keepAliveCount >= 10) {
                echo "event: keep-alive\n";
                echo "data: \n\n";
                ob_flush();
                flush();
                $keepAliveCount = 0;
            }

            $keepAliveCount++;
            usleep(100000); // Sleep for 100 milliseconds (0.1 seconds)
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
