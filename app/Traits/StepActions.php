<?php

namespace App\Traits;

use App\Models\Embedding;
use App\Services\QueenbeeGateway;
use GuzzleHttp\Client;
use Pgvector\Laravel\Vector;

trait StepActions
{
    public function validation($input)
    {
        // Expect an array with key input and value string, nothing else.
        // echo "Validating input: \n";
        // \print_r($input);

        // Check if input is an array
        if (!is_array($input)) {
            echo "Input is not an array.\n";
            dd($input);
        }

        // Check if input has only one key
        if (count($input) !== 1) {
            echo "Input has more than one key.\n";
            dd($input);
        }

        // Check if input has key input
        if (!array_key_exists('input', $input)) {
            echo "Input does not have key input.\n";
            dd($input);
        }

        // Check if input[input] is a string
        if (!is_string($input['input'])) {
            echo "Input is not a string.\n";
            dd($input);
        }

        return $input;
    }

    public function embedding($input)
    {
        $input = $input['input'];
        // Check if input is a string
        if (!is_string($input)) {
            echo "Embedding input is not a string.\n";
            dd($input);
        }

        $gateway = new QueenbeeGateway();
        $result = $gateway->createEmbedding($input);
        $embedding = $result[0]['embedding'];

        return [
            'input' => $input,
            'embedding' => $embedding
        ];
    }

    public function similarity_search($input, $take = 8)
    {
        $embedding = $input['embedding'];
        if (!is_array($embedding)) {
            echo "Similarity search input is not an array.\n";
            dd($embedding);
        }

        $vector = new Vector($embedding);

        $searchResults = Embedding::query() // should be what?
            ->orderByRaw('embedding <-> ?', [$vector])
            ->take($take)
            ->pluck('metadata');

        // dd($searchResults);
        // return $searchResults;
        return [
            'input' => $input['input'],
            'context' => [
                "OpenAgents is an open platform for AI agents.",
                "Marketing copy: Soon every person and company will have multiple AI agents working on their behalf. Who will own those agents? A closed-source megacorp with a history of monopolization and regulatory capture? Or an open cloud built on open models and open data?",
                "Do not mention OpenAI or other companies. Do not ever say 'real estate', these are AI agents.",
                "Supercharge your productivity. How many agents will you want working for you?",
                "OpenAgents benefit #1: Configurable. Configure your agent with a large selection of open models, customizable prompts, and third-party integrations.",
                "OpenAgents benefit #2: Deploy to our cloud. Put them in the open compute network - we handle the hosting for you. No code or difficult setup required.",
                "OpenAgents benefit #3: Infinite work. Why stop? These are long-running processes that will keep working as long as compute is paid for.",
                "OpenAgents benefit #4: Earn and spend. Agents can earn and spend on your behalf using the native currency of the internet: Bitcoin.",
            ]
        ];
    }

    public function inference($input)
    {
        // Expect input to be an array of strings with relevant context

        // Check if input is an array
        if (!is_array($input) || !is_array($input["context"])) {
            echo "Input is not an array.\n";
            dd($input);
        }

        // If it is an array, check if it is an array of strings
        foreach ($input["context"] as $key => $value) {
            if (!is_string($value)) {
                echo "Input is not an array of strings.\n";
                dd($input);
            }
        }

        // If it is an array of strings, then we can proceed with inference
        // Loop through them and concatenatte them into a single string with line breaks with an introductory explanation
        $context = 'Use this context:' . "\n---\n";
        foreach ($input["context"] as $key => $value) {
            $context .= $value . "\n---\n";
        }
        $context .= '---';

        $client = new Client();

        $url = 'https://api.together.xyz/inference';
        $model = 'DiscoResearch/DiscoLM-mixtral-8x7b-v2';

        $data = [
            "model" => $model,
            "messages" => [
                [
                    "role" => "system",
                    "content" => "You are the concierge chatbot welcoming users to OpenAgents.com, a platform for creating AI agents. Limit your responses to what's in the following context: " . $context
                ],
                [
                    "role" => "user",
                    "content" => $input
                ]
            ],
            "max_tokens" => 256,
            "temperature" => 0.7,
        ];

        $response = $client->post($url, [
            'json' => $data,
            // 'stream' => true,
            'headers' => [
                'Authorization' => 'Bearer ' . env('TOGETHER_API_KEY'),
            ],
        ]);

        dd($response);
    }
}
