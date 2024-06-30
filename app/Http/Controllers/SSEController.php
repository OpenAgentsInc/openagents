<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Support\Facades\Response;

class SSEController extends Controller
{
    public function stream(Request $request)
    {
        $userMessage = $request->input('message', '');

        return Response::stream(function() use ($userMessage) {
            if (ob_get_level()) ob_end_clean();

            header('Content-Type: text/event-stream');
            header('Cache-Control: no-cache');
            header('Connection: keep-alive');
            header('X-Accel-Buffering: no');

            // Generate a response based on user input
            $response = $this->generateResponse($userMessage);
            $tokens = explode(' ', $response);

            foreach ($tokens as $token) {
                echo "event: message\n";
                echo "data: " . json_encode(['type' => 'token', 'content' => $token . ' ']) . "\n\n";

                if (ob_get_level() > 0) ob_flush();
                flush();

                usleep(100000); // 0.1 second delay between tokens
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

    private function generateResponse($userMessage)
    {
        // Simple demo response generator
        $responses = [
            "Hello! How can I assist you today?",
            "That's an interesting question. Let me think about it.",
            "I understand your concern. Here's what I think:",
            "Based on what you've said, I would suggest the following:",
            "Thank you for sharing that. Here's my perspective:",
        ];

        $baseResponse = $responses[array_rand($responses)];
        return $baseResponse . " You said: '" . $userMessage . "'. Is there anything else you'd like to know?";
    }
}
