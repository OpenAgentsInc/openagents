<?php

namespace App\Services;

class Planner
{
    private $gateway;

    public function __construct()
    {
        $this->gateway = new OpenAIGateway();
    }

    public function createPlan(array $messages): string
    {
        $plan = '';

        // Create the string to pass to LLM
        foreach ($messages as $message) {
            $plan .= $message['role'] === 'assistant' ? "Faerie said: \n" : "User said: \n";
            $plan .= $message['content'] . "\n\n";

        }

        $prompt = "Based on the following conversation, please respond with a step-by-step plan for our junior developer to follow. Be extremely specific about code needs to be added, what files need to be edited, what tests need to be written, and the expected outcome. But do not write any of the code! You are just providing instructions to the junior developer. Do not give any general advice. Only give specific tasks that can be completed with code. \n\n-----\n\n" . $plan;
        $plan = $this->complete($prompt);

        return $plan;
    }

    public function formatIssueAndCommentsAsMessages(string $issueBody, array $commentsResponse): array
    {
        $messages = [];

        $messages[] = ['role' => 'user', 'content' => $issueBody];

        foreach ($commentsResponse as $comment) {
            $role = $comment['user']['login'] === 'FaerieAI' ? 'assistant' : 'user';
            $messages[] = ['role' => $role, 'content' => $comment['body']];
        }

        return $messages;
    }

    public function complete($prompt)
    {
        $messages = [
            ['role' => 'system', 'content' => "You are Faerie, an AI agent specialized in writing & analyzing code. Respond concisely and in a way that a senior developer would respond. Don't introduce yourself or use flowery text or a closing signature."],
            ['role' => 'user', 'content' => $prompt],
        ];

        // Make the chat completion to generate the comment
        $response = $this->gateway->makeChatCompletion([
            'model' => 'gpt-4',
            'messages' => $messages,
        ]);
        $final = $response['choices'][0]['message']['content'];
        return $final;
    }
}
