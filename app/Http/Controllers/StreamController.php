<?php

namespace App\Http\Controllers;

use App\Events\ChatTokenReceived;
use App\Http\StreamResponse;
use App\Models\Conversation;
use App\Models\Message;
use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;
use GuzzleHttp\Psr7\Request;
use Illuminate\Support\Facades\Http;

class StreamController extends Controller
{
    public function chat()
    {
        request()->validate([
            'input' => 'required',
        ]);

        $input = request('input');

        $conversation = Conversation::create([
            'user_id' => auth()->user()->id ?? 1,
        ]);

        try {
            $client = new Client();

            $url = 'https://api.together.xyz/inference';
            $model = 'DiscoResearch/DiscoLM-mixtral-8x7b-v2';

            $data = [
                "model" => $model,
                "messages" => [
                    [
                        "role" => "system",
                        "content" => "You are the concierge chatbot welcoming users to OpenAgents.com. Limit your responses to short explanations. Whatever you don't know about, or if you feel something is out of scope for you as a concierge, encourage the user to explore the OpenAgents site. Maybe they can find an agent to answer their question, or can create their own and earn bitcoin rewards based on the usage of the agent."
                    ],
                    [
                        "role" => "user",
                        "content" => $input
                    ]
                ],
                "max_tokens" => 256,
                "temperature" => 0.7,
                "stream_tokens" => true
            ];

            $response = $client->post($url, [
                'json' => $data,
                'stream' => true,
                'headers' => [
                    'Authorization' => 'Bearer ' . env('TOGETHER_API_KEY'),
                ],
            ]);

            // Reading the streamed response
            $stream = $response->getBody();

            $message = Message::create([
                'conversation_id' => $conversation->id,
                'user_id' => $conversation->user_id,
                'body' => "",
                'sender' => 'agent',
            ]);

            $content = "";
            $tokenId = 0;

            foreach ($this->readStream($stream) as $responseLine) {
                $token = $responseLine["choices"][0]["text"];
                $content .= $token;
                broadcast(new ChatTokenReceived($token, $message->id, $tokenId++));
            }

            $message->update([
                'body' => $content,
            ]);
        } catch (RequestException $e) {
            // Handle exception or errors here
            echo $e->getMessage();
        }
    }

    public function readStream($stream)
    {
        while (!$stream->eof()) {
            $line = $this->readLine($stream);

            if (!str_starts_with($line, 'data:')) {
                continue;
            }

            $data = trim(substr($line, strlen('data:')));

            if ($data === '[DONE]') {
                break;
            }

            $response = json_decode($data, true, JSON_THROW_ON_ERROR);

            if (isset($response['error'])) {
                throw new \Exception($response['error']);
            }

            yield $response;
        }
    }

    public function readLine($stream)
    {
        $line = '';
        while (!$stream->eof()) {
            $char = $stream->read(1);
            if ($char === "\n") {
                break;
            }
            $line .= $char;
        }
        return $line;
    }
}
