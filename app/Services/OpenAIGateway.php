<?php

namespace App\Services;

use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Http;
use OpenAI;

class OpenAIGateway
{

    private $api_url;
    private $client;
    private $token;

    public function __construct()
    {
        $this->api_url = "https://api.openai.com/v1";
        $this->token = env('OPENAI_API_KEY');
        // $this->client = OpenAI::client($this->token);
    }

    public function defaultModel()
    {
        return 'gpt-3.5-turbo';
    }

    public function makeChatCompletion($data)
    {
        // dump("About to go do a thing");
        // dump($data);
        // dd($data);

        // $data = [
        //     "model" => "gpt-4",
        //     "messages" => [
        //         [
        //             "role" => "system",
        //             "content" => "Hello, I'm a chatbot that can help you find files. What would you like to search for?"
        //         ],
        //         [
        //             "role" => "user",
        //             "content" => "I'm looking for a file about the new product launch."
        //         ]
        //     ],
        // ];

        // dd(json_encode($data));

        $response = Http::withHeaders([
            'Authorization' => 'Bearer ' . $this->token,
            'Content-Type' => 'application/json',
            'Accept' => 'application/json'
        ])
            ->timeout(90)
            ->post($this->api_url . '/chat/completions', $data);

        // $this->filterNonUtf8CharsFromArray($data)

        // dump("Did what");

        // dd($response->json());
        // dump("RESPONSE:");
        // dump($response);
        // dump("RESPONSE JSON:");
        // dump($response->json());

        return $response->json();
    }

    // function filterNonUtf8Characters($data) {
    //     foreach ($data as &$item) {
    //         if (isset($item['content'])) {
    //             // Convert to UTF-8, substituting non-UTF-8 characters
    //             $item['content'] = mb_convert_encoding($item['content'], 'UTF-8', 'UTF-8');
    //         }
    //     }
    //     return $data;
    // }

    public function filterNonUtf8CharsFromArray($data)
    {
        foreach ($data as &$item) {
            if (isset($item['content'])) {
                // Remove non-UTF-8 characters
                $item['content'] = preg_replace('/[\x00-\x1F\x80-\xFF]/', '', $item['content']);
                $item['content'] = mb_convert_encoding($item['content'], 'UTF-8', 'UTF-8');
            }
        }
        return $data;
    }
}
