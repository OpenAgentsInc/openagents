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
        $this->keepAlive();
    }

    public function store()
    {
        $input = request('message-input');
        $thread_id = request('thread_id');

        $this->addMessageToQueue('<br />'.$input.'<br />');

        $this->processInference($input, $thread_id);

        return response()->noContent();
    }

    private function addMessageToQueue($message)
    {
        $messages = Cache::get('message_queue', []);
        $messages[] = $message;
        Cache::put('message_queue', $messages);
    }

    private function processInference($input, $thread_id)
    {
        $thread = Thread::findOrFail($thread_id);
        $inference = new SimpleInferencer();
        $inference->inference($input, 'gpt-4o', $thread, function ($content) {
            $this->addMessageToQueue($content);
        });
    }
}
