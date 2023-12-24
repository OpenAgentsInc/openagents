<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Support\Facades\Http;
use Textalk\SseClient;

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

        $sseClient = new SseClient($response->body());

        $done = false;

        foreach ($sseClient->read() as $event) {
            if ($event->data === "[DONE]") {
                $done = true;
                break;
            }

            // Process the streaming token data here
            $partialResult = json_decode($event->data, true);
            $token = $partialResult["choices"][0]["text"];
            // You can do something with $token here (e.g., save to a database, return as a response, etc.)
        }

        if ($done) {
            // Final message handling here
        }
    }
}
