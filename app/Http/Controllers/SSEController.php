<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\StreamedResponse;

class SSEController extends Controller
{
    public function stream(Request $request)
    {
        return new StreamedResponse(function() {
            // Simulate token streaming (replace with your actual LLM integration)
            $tokens = ['Hello', ', ', 'how', ' ', 'are', ' ', 'you', '?'];

            foreach ($tokens as $token) {
                echo "data: " . json_encode(['type' => 'token', 'content' => $token]) . "\n\n";
                ob_flush();
                flush();
                usleep(100000); // Simulate delay between tokens
            }

            echo "data: " . json_encode(['type' => 'end']) . "\n\n";
            ob_flush();
            flush();
        }, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
        ]);
    }
}
