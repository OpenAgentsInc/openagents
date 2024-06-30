<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Support\Facades\Response;
use App\Services\AnthropicService;
use Illuminate\Support\Facades\Log;

class SSEController extends Controller
{
    protected $anthropicService;

    public function __construct(AnthropicService $anthropicService)
    {
        $this->anthropicService = $anthropicService;
    }

    public function stream(Request $request)
    {
        $userMessage = $request->input('message', '');

        return Response::stream(function() use ($userMessage) {
            // Disable output buffering
            if (ob_get_level() > 0) {
                ob_end_clean();
            }

            // Set headers
            header('Content-Type: text/event-stream');
            header('Cache-Control: no-cache');
            header('Connection: keep-alive');
            header('X-Accel-Buffering: no');

            // Send an initial message to establish the connection
            echo "event: connection\n";
            echo "data: Connected\n\n";
            flush();

            $this->anthropicService->streamResponse($userMessage, function($data) {
                echo "event: message\n";
                echo "data: " . json_encode($data) . "\n\n";

                flush();

                Log::info('Sent data to client', ['data' => $data]);
            });

            // Send a final message to close the connection
            echo "event: close\n";
            echo "data: Stream closed\n\n";
            flush();
        }, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'Connection' => 'keep-alive',
            'X-Accel-Buffering' => 'no',
        ]);
    }
}
