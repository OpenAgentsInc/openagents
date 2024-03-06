<?php

namespace App\Services;

use App\AI\GroqAIGateway;
use App\Models\Agent;
use App\Models\Thread;

class Inferencer
{
    public static function llmInference($input, Thread $thread, Agent $agent, $streamFunction)
    {
        $model = 'mixtral-8x7b-32768';
        $messages = self::prepareTextInference($input, $thread, $agent);
        //            $client = new MistralAIGateway();
        $client = new GroqAIGateway();
        $content = $client->chat()->createStreamed([
            'model' => $model,
            'messages' => $messages,
            'max_tokens' => 3024,
            'stream_function' => $streamFunction,
        ]);

        return ['output' => $content];
    }

    private static function prepareTextInference($text, Thread $thread, Agent $agent)
    {
        // Fetch previous messages
        $previousMessages = $thread->messages()
            ->orderBy('created_at', 'asc')
            ->get()
            ->map(function ($message) {
                // Map 'user' and 'agent' to 'user' and 'assistant' respectively
                $role = $message->sender === 'agent' ? 'assistant' : 'user';

                return [
                    'role' => $role,
                    'content' => $message->body,
                ];
            })
            ->toArray();

        // Prepend system message
        array_unshift($previousMessages, [
            'role' => 'system',
            'content' => 'You are a helpful AI agent named '.$agent->name.' 
            
Your description is: '.$agent->description.'

Your instructions are: 
---
'.$agent->instructions.'
---',
        ]);

        return $previousMessages;
    }
}
