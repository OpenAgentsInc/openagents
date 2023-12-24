<?php

namespace App\Http\Controllers;

use App\Http\StreamResponse;
use App\Events\ChatTokenReceived;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Http;
use Nyholm\Psr7\Factory\Psr17Factory;

class StreamController extends Controller
{
    public function streamTokens(Request $request)
    {
        $apiUrl = 'https://api.together.xyz/inference';
        $model = 'togethercomputer/RedPajama-INCITE-7B-Instruct';

        $payload = [
            "model" => $model,
            "prompt" => "Alan Turing was",
            "max_tokens" => 128,
            "stop" => ["\n\n"],
            "temperature" => 0.7,
            "top_p" => 0.7,
            "top_k" => 50,
            "repetition_penalty" => 1,
            "stream_tokens" => true
        ];

        $response = Http::withHeaders([
            'Authorization' => 'Bearer ' . env('TOGETHER_API_KEY'),
            'Content-Type' => 'application/json',
        ])->post($apiUrl, $payload);

        // $response->throw();

        // Convert Laravel HTTP client response to PSR-7 response
        $psr17Factory = new Psr17Factory();
        $psrResponse = $psr17Factory->createResponse(
            $response->status(),
            $response->body(),
            $response->headers()
        );

        $streamResponse = new StreamResponse($psrResponse);
        foreach ($streamResponse->getIterator() as $tokenData) {
            // Check for final message
            if (isset($tokenData['data']) && $tokenData['data'] === '[DONE]') {
                // Final message handling here
                break;
            }

            // Process the streaming token data here
            $token = $tokenData["choices"][0]["text"];
            // You can do something with $token here (e.g., save to a database, return as a response, etc.)

            // Broadcast to the Chat channel
            broadcast(new ChatTokenReceived($token));
        }
    }
}
