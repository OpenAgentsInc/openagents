<?php

namespace App\Http\Controllers;

use App\Events\ChatTokenReceived;
use App\Http\Controllers\StreamController;
use App\Models\Conversation;
use App\Models\Message;
use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;
use GuzzleHttp\Psr7\Request;
use Illuminate\Support\Facades\Http;
use Inertia\Inertia;

class StreamController extends Controller
{
    public function chat()
    {
        return redirect()->route('agent.chat', ['id' => 1]);
    }

    // public function stream()
    // {
    //     request()->validate([
    //         'input' => 'required',
    //     ]);

    //     $input = request('input');

    //     $this->doChat($input);
    // }

    public function doChat($input, $conversation = null, $context = "")
    {
        // Define the system prompt for the assistant
        $systemPrompt = "You are a helpful AI agent on OpenAgents.com. Answer the user question based on the following context: " . $context;

        if (!$conversation) {
            // If no conversation is provided, use a basic system prompt and perform inference
            $messages = [
                [
                    "role" => "system",
                    "content" => $systemPrompt,
                ],
                ["role" => "user", "content" => $input],
            ];
        } else {
            // Fetch the 15 most recent conversation messages sorted in chronological order oldest to newest
            $previousMessages = Message::where('conversation_id', $conversation->id)
                ->orderBy('created_at', 'asc')
                ->take(15)
                ->get()
                ->toArray();

            // Initialize messages with the system prompt and previous messages
            $messages = [
                [
                    "role" => "system",
                    "content" => $systemPrompt,
                ],
            ];

            foreach ($previousMessages as $msg) {
                $messages[] = [
                    "role" => $msg['sender'] === 'user' ? 'user' : 'assistant',
                    "content" => $msg['body']
                ];
            }

            // Add the current user input as the last element
            $messages[] = ["role" => "user", "content" => $input];
        }

        // Perform inference with the messages
        $output = $this->performInference($messages);

        if ($conversation) {
            // If there's a conversation, save the agent's response as a message
            $this->saveAgentResponse($conversation, $output);
        }

        return $output;
    }

    private function performInference(array $messages)
    {
        try {
            $client = new Client();

            $url = 'https://api.together.xyz/inference';
            $model = 'DiscoResearch/DiscoLM-mixtral-8x7b-v2';

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

            $content = "";
            $tokenId = 0;

            foreach ($this->readStream($stream) as $responseLine) {
                $token = $responseLine["choices"][0]["text"];
                $content .= $token;
            }

            return $content;
        } catch (RequestException $e) {
            // Handle exception or errors here
            echo $e->getMessage();
        }
    }

    private function saveAgentResponse($conversation, $content)
    {
        // Create a message to store the agent's response
        Message::create([
            'conversation_id' => $conversation->id,
            'user_id' => $conversation->user_id,
            'body' => $content,
            'sender' => 'agent',
        ]);
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
