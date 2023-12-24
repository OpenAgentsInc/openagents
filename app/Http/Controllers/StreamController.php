<?php

namespace App\Http\Controllers;

use App\Http\StreamResponse;
use App\Events\ChatTokenReceived;
use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;
use GuzzleHttp\Psr7\Request;
// use Illuminate\Http\Request;
use Illuminate\Support\Facades\Http;

// use Illuminate\Support\Facades\Log;
// use Nyholm\Psr7\Factory\Psr17Factory;

$client = new Client();

class StreamController extends Controller
{
    public function streamTokens()
    {
        try {
            $url = 'https://api.together.xyz/inference';
            $model = 'togethercomputer/RedPajama-INCITE-7B-Instruct';

            $data = [
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

            $response = $client->post($url, [
                'json' => $data,
                'stream' => true, // Enable streaming
            ]);

            // Reading the streamed response
            $stream = $response->getBody();
            while (!$stream->eof()) {
                echo $stream->read(1024);
            }
        } catch (RequestException $e) {
            // Handle exception or errors here
            echo $e->getMessage();
        }

    }


    public function streamTokens2(Request $request)
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

        dump($payload);

        $response = Http::withHeaders([
            'Authorization' => 'Bearer ' . env('TOGETHER_API_KEY'),
            'Content-Type' => 'application/json',
        ])->withOptions([
            'stream' => true,
        ])
        ->post($apiUrl, $payload);




        if ($response->successful()) {
            dump("Response was successful");
            // Open a stream to read the response
            $stream = $response->toStream();

            // Read the stream content chunk by chunk
            while (!$stream->eof()) {
                dump('in here');
                $chunk = $stream->read(1024); // Read 1 KB at a time (adjust as needed)
                // Process or output the chunk as needed
                dump($chunk);
                broadcast(new ChatTokenReceived($chunk));
                echo $chunk;
            }

            // Close the stream when done
            $stream->close();
        } else {
            dump("Response was not successful");
            // Handle the HTTP error response
            $statusCode = $response->status();
            dump($statusCode);
            $errorMessage = $response->body();
            dump($errorMessage);
            // Handle the error as needed
            // You can log, throw exceptions, or perform other error handling here
        }




        // dump("Did request");

        // $response->throw();

        // Convert Laravel HTTP client response to PSR-7 response
        // $psr17Factory = new Psr17Factory();
        // $psrResponse = $psr17Factory->createResponse(
        //     $response->status(),
        //     $response->body(),
        //     $response->headers()
        // );

        // dump('did psr thing');
        // $body = $response->toPsrResponse()->getBody();
        // dump('got body');
        // dump($body);

        // $streamResponse = new StreamResponse($body);
        // dump('got psr response');
        // foreach ($streamResponse->getIterator() as $tokenData) {

        //     dump('in a loop');
        //     dump($tokenData);

        //     // Check for final message
        //     if (isset($tokenData['data']) && $tokenData['data'] === '[DONE]') {
        //         // Final message handling here
        //         break;
        //     }

        //     // Process the streaming token data here
        //     $token = $tokenData["choices"][0]["text"];
        //     // You can do something with $token here (e.g., save to a database, return as a response, etc.)

        //     // Broadcast to the Chat channel
        //     broadcast(new ChatTokenReceived($token));
        // }
    }
}
