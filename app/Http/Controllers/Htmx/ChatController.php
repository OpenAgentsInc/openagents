<?php

namespace App\Http\Controllers\Htmx;

use App\AI\SimpleInferencer;
use App\Http\Controllers\Controller;
use App\Models\Thread;
use App\Services\LocalLogger;
use App\Traits\Streams;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Session;

class ChatController extends Controller
{
    use Streams;

    public function sseStreamTest()
    {
        return view('htmx.sse-stream');
    }

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
        $lock = Cache::lock('message_queue_lock', 10); // 10-second lock

        try {
            if ($lock->get()) {
                // Retrieve the messages, update the queue, and store it back
                $messages = Cache::get('message_queue', []);
                $messages[] = $message;
                Cache::put('message_queue', $messages);
            } else {
                // Log if unable to acquire the lock
                Log::warning('Unable to acquire lock for updating message queue');
            }
        } finally {
            $lock->release();
        }
    }

    private function processInference($input, $thread_id)
    {
        $thread = Thread::findOrFail($thread_id);

        // Authenticate user session or proceed without it
        $sessionId = auth()->check() ? null : Session::getId();

        // Save user message to the thread
        $thread->messages()->create([
            'body' => $input,
            'session_id' => $sessionId,
            'user_id' => auth()->id() ?? null,
            'input_tokens' => null,
            'output_tokens' => null,
        ]);

        $inference = new SimpleInferencer();
        $logger = new LocalLogger();
        $output = $inference->inference($input, 'gpt-4o', $thread, function ($content) {
            $inferenceMessage = '<span>'.nl2br(e($content)).'</span>';
            //            $inferenceMessage = '<pre>'.htmlspecialchars($content).'</pre>';

            //            $inferenceMessage = nl2br(e($content));

            $this->addMessageToQueue($inferenceMessage);
        });

        // Append the response to the chat
        $message = [
            'body' => $output['content'],
            'model' => 'gpt-4o',
            'user_id' => auth()->id() ?? null,
            'session_id' => $sessionId,
            'agent_id' => null,
            'agent' => null,
            'input_tokens' => $output['input_tokens'],
            'output_tokens' => $output['output_tokens'],
        ];

        // Save the agent's response to the thread
        $thread->messages()->create($message);
    }
}
