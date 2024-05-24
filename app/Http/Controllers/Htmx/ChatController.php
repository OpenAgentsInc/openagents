<?php

namespace App\Http\Controllers\Htmx;

use App\AI\SimpleInferencer;
use App\Http\Controllers\Controller;
use App\Models\Thread;
use App\Traits\Streams;
use Illuminate\Support\Facades\Session;

class ChatController extends Controller
{
    use Streams;

    public function creditBalance()
    {
        // Assuming you have a User model with a credits attribute
        $credits = auth()->user()->credits;

        // Return the credit balance as a simple plain text response
        return response($credits);
    }

    public function payme()
    {
        // Pick a random integer between 1 and 50
        $randomNumber = random_int(1, 50);

        // Increment the user's credits by the random number
        auth()->user()->increment('credits', $randomNumber);

        $this->logger->log("Incremented user's credits by $randomNumber sats");

        // Then stream the updated balance
        $this->stream('BalanceUpdate', auth()->user()->credits);
        //        $this->logger->log('Streamed updated balance to the user '.$randomNumber);
        //        $this->stream('StatusMessage', 'You got paid '.$randomNumber.' sats!');
        //        $this->logger->log('Streamed status message to the user '.$randomNumber);

        return response()->noContent();
    }

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
        $this->startStream();
    }

    public function store()
    {
        $input = request('message-input');
        $thread_id = request('thread_id');

        $this->stream('<br />'.$input.'<br />');

        $this->processInference($input, $thread_id);

        return response()->noContent();
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

        $output = $inference->inference($input, 'gpt-4o', $thread, function ($content) {
            $inferenceMessage = '<span>'.nl2br(e($content)).'</span>';
            //            $inferenceMessage = '<pre>'.htmlspecialchars($content).'</pre>';
            //            $inferenceMessage = nl2br(e($content));

            $this->stream($inferenceMessage);
            //            $this->addMessageToQueue($inferenceMessage);
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
