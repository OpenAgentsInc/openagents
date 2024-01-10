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

    public function stream()
    {
        request()->validate([
            'input' => 'required',
        ]);

        $input = request('input');

        $this->doChat($input);
    }

    public function fetchOrCreateConversation()
    {
        // Check session for conversation id
        $conversationId = session('conversation_id');

        $conversation = null;

        if ($conversationId) {
            $conversation = Conversation::find($conversationId);
        }

        if (!$conversation) {
            $conversation = Conversation::create([
                'agent_id' => $agentId,
                'user_id' => auth()->user()->id ?? null,
            ]);

            // set the session
            session(['conversation_id' => $conversation->id]);
        }

        return $conversation;
    }


    public function doChat($input, $context = "")
    {
        $conversation = $this->fetchOrCreateConversation();

        // Fetch the 15 most recent conversation messages sorted in chronological order oldest to newest
        $previousMessages = Message::where('conversation_id', $conversation->id)
            ->orderBy('created_at', 'asc')
            ->take(15)
            ->get()
            ->toArray();

        $systemPrompt = "You are the concierge chatbot welcoming users to OpenAgents.com, a platform for creating AI agents. Limit your responses to what's in the following context: " . $context;

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
                    "content" => $systemPrompt,
                ],
                [
                    "role" => "assistant",
                    "content" => "Welcome! I am Concierge, the first OpenAgent.\n\nYou can ask me basic questions about OpenAgents and I will try my best to answer.\n\nClick 'Agent' on the left to see what I know and how I act.\n\nI might lie or say something crazy. Oh well - thank you for testing!"
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
