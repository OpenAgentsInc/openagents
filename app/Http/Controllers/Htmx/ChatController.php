<?php

namespace App\Http\Controllers\Htmx;

use App\AI\SimpleInferencer;
use App\Http\Controllers\Controller;
use App\Models\Thread;

class ChatController extends Controller
{
    public function index()
    {
        return view('htmx.chat');
    }

    public function sseStream()
    {
        $keepAlive = function () {
            while (true) {
                echo "event: keep-alive!!\n";
                echo "data: \n\n";
                ob_flush();
                flush();
                sleep(5);
            }
        };

        $this->initializeStream($keepAlive);
    }

    private function initializeStream($keepAlive)
    {
        $response = response()->stream($keepAlive, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'X-Accel-Buffering' => 'no',
        ]);

        $response->send();
    }

    private function stream()
    {

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
}
