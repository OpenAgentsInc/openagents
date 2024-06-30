<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Support\Facades\Response;

class SSEController extends Controller
{
    public function stream(Request $request)
    {
        return Response::stream(function() {
            // Disable output buffering
            if (ob_get_level()) ob_end_clean();

            // Set headers
            header('Content-Type: text/event-stream');
            header('Cache-Control: no-cache');
            header('Connection: keep-alive');
            header('X-Accel-Buffering: no');

            // Simulate token streaming (replace with your actual LLM integration)
            $tokens = ['Hello', ', ', 'how', ' ', 'are', ' ', 'you', '?'];

            foreach ($tokens as $token) {
                echo "event: message\n";
                echo "data: " . json_encode(['type' => 'token', 'content' => $token]) . "\n\n";

                if (ob_get_level() > 0) ob_flush();
                flush();

                usleep(150000); // Simulate delay between tokens
            }

            echo "event: message\n";
            echo "data: " . json_encode(['type' => 'end']) . "\n\n";

            if (ob_get_level() > 0) ob_flush();
            flush();
        }, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'Connection' => 'keep-alive',
            'X-Accel-Buffering' => 'no',
        ]);
    }
}
