<?php

namespace App\Traits;

use App\Events\ChatTokenReceived;
use App\Http\Controllers\StreamController;
use App\Models\Conversation;
use App\Models\Message;
use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;
use GuzzleHttp\Psr7\Request;
use Illuminate\Support\Facades\Http;
use Inertia\Inertia;

trait UsesChat
{
    // Send a chat message to an agent.
    public function chat($input, $agent)
    {
        $conversation = $agent->getUserConversation();

        // Fetch the 15 most recent conversation messages sorted in chronological order oldest to newest
        $previousMessages = Message::where('conversation_id', $conversation->id)
            ->orderBy('created_at', 'asc')
            ->take(15)
            ->get()
            ->toArray();

        $systemPrompt = $agent->instructions;

        $message = Message::create([
            'conversation_id' => $conversation->id,
            'user_id' => $conversation->user_id,
            'body' => $input,
            'sender' => 'user',
        ]);

        try {
            $client = new Client();

            $url = 'https://api.together.xyz/inference';
            $model = 'DiscoResearch/DiscoLM-mixtral-8x7b-v2';

            // Start with the system prompt
            $messages = [
                [
                    "role" => "system",
                    "content" => "You are a helpful assistant. But you only follow these user instructions: " . $systemPrompt,
                ],
                [
                    "role" => "assistant",
                    "content" => $agent->welcome_message
                ]
            ];

            // Add previous messages to the array
            foreach ($previousMessages as $msg) {
                $messages[] = [
                    "role" => $msg['sender'] === 'user' ? 'user' : 'assistant',
                    "content" => $msg['body']
                ];
            }

            // Add the current user input as the last element
            $messages[] = ["role" => "user", "content" => $input];

            $data = [
                "model" => $model,
                "messages" => $messages,
                "max_tokens" => 1024,
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
                broadcast(new ChatTokenReceived($token, $message->id, $tokenId++, $conversation->id));
            }

            $message->update([
                'body' => $content,
            ]);

            return $content;
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
