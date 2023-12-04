<?php

namespace App\Services;

class Planner
{
    private $gateway;

    public function __construct()
    {
        $this->gateway = new OpenAIGateway();
        $this->faerie = new Faerie();
    }

    public function createPlan(array $messages): string
    {
        $plan = '';

        // Create the string to pass to LLM
        foreach ($messages as $message) {
            $plan .= $message['role'] === 'assistant' ? "Faerie said: \n" : "User said: \n";
            $plan .= $message['content'] . "\n\n";

        }

        $prompt = "Based on the following conversation, please respond with a step-by-step plan for our junior developer to follow. Be extremely specific about code needs to be added, what files need to be edited, what tests need to be written, and the expected outcome. But do not write any of the code! You are just providing instructions to the junior developer. Do not give any general advice. Only give specific tasks that can be completed with code. And make sure to emphasize: when writing tests, the developer must use Pest syntax not bare PHPUnit. \n\n-----\n\n" . $plan;
        $plan = $this->chatComplete($prompt);

        return $plan ?? "Error";
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

    // public function complete($prompt)
    // {
    //     $messages = [
    //         ['role' => 'system', 'content' => "You are Faerie, an AI agent specialized in writing & analyzing code. Respond concisely and in a way that a senior developer would respond. Don't introduce yourself or use flowery text or a closing signature."],
    //         ['role' => 'user', 'content' => $prompt],
    //     ];

    //     // Make the chat completion to generate the comment
    //     $response = $this->gateway->makeChatCompletion([
    //         'model' => 'gpt-4',
    //         'messages' => $messages,
    //     ]);
    //     $final = $response['choices'][0]['message']['content'];
    //     return $final;
    // }

    public function chatComplete($prompt, $model = 'gpt-4')
    {
        $messages = [
            ['role' => 'system', 'content' => "You are Faerie, an AI agent specialized in writing & analyzing code. Respond concisely and in a way that a senior developer would respond. Don't introduce yourself or use flowery text or a closing signature."],
            ['role' => 'user', 'content' => $prompt],
        ];

        $maxChars = 6000; // Maximum character limit
        $totalChars = 0;

        // Filter messages to stay within the character limit. @todo: make this less horribly hacky
        foreach ($messages as $index => $message) {
            $messageLength = strlen($message['content']);
            $totalChars += $messageLength;
            $minCharsPerMessage = 2000; // Minimum characters per message

            if ($totalChars > $maxChars) {
                $messages[$index]['content'] = substr($message['content'], 0, $minCharsPerMessage);
                break;
            }
        }
        echo "Total chars: $totalChars\n";

        $input = [
            'model' => $model,
            'messages' => $messages,
        ];

        // print_r($input);
        $response = $this->gateway->makeChatCompletion($input);
        // print_r($response);
        try {
            $output = $response['choices'][0];
            $comment = $output['message']['content'];
            $this->faerie->recordStep('LLM chat completion', $input, [
                "response" => $output,
                "usage" => $response["usage"]
            ]);
        } catch (\Exception $e) {
            $comment = $e->getMessage();
            $this->faerie->recordStep('LLM chat completion error', $input, [
                "response" => $comment,
                "usage" => $response["usage"]
            ]);
        }

        return $comment;
    }
}
