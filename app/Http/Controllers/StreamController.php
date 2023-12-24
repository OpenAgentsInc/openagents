<?php

namespace App\Http\Controllers;

use App\Events\ChatTokenReceived;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Http;

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

        $response->throw();

        $done = false;

        foreach ($response->body() as $line) {
            // Parse the streaming token data here
            $partialResult = json_decode($line, true);
            $token = $partialResult["choices"][0]["text"];
            // You can do something with $token here (e.g., save to a database, return as a response, etc.)

            // Broadcast to the Chat channel
            broadcast(new ChatTokenReceived($token));
        }

        if ($done) {
            // Final message handling here
        }
    }
}
