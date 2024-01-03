<?php

namespace App\Traits;

use App\Http\Controllers\StreamController;
use App\Models\Datapoint;
use App\Models\Embedding;
use App\Services\QueenbeeGateway;
use App\Services\OpenAIGateway;
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
        // if (!is_array($input)) {
        //     echo "Input is not an array.\n";
        //     dd($input);
        // }

        // // Check if input has only one key
        // if (count($input) !== 1) {
        //     echo "Input has more than one key.\n";
        //     dd($input);
        // }

        // // Check if input has key input
        // if (!array_key_exists('input', $input)) {
        //     echo "Input does not have key input.\n";
        //     dd($input);
        // }

        // // Check if input[input] is a string
        // if (!is_string($input['input'])) {
        //     echo "Input is not a string.\n";
        //     dd($input);
        // }

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

        $searchResults = Datapoint::query()
            // ->where('brain_id', 1) -- For now using ALL DATAPOINTS
            ->orderByRaw('embedding <-> ?', [$vector])
            ->take($take)
            ->pluck('data');

        // dd($searchResults->toArray());
        return [
            'input' => $input['input'],
            'context' => $searchResults->toArray()
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

        // $gateway = new OpenAIGateway();

        // Initiate new StreamController
        $streamer = new StreamController();
        $last = $streamer->doChat($input["input"], $context);


        // $data = [
        //     "model" => $gateway->defaultModel(),
        //     "messages" => [
        //         [
        //             "role" => "system",
        //             "content" => "You are the concierge chatbot welcoming users to OpenAgents.com, a platform for creating AI agents. Limit your responses to what's in the following context: " . $context
        //         ],
        //         [
        //             "role" => "user",
        //             "content" => $input['input']
        //         ]
        //     ],
        //     "max_tokens" => 256,
        //     "temperature" => 0.7,
        // ];

        // $chatResponse = $gateway->makeChatCompletion($data);
        // $last = $chatResponse["choices"][0]["message"]["content"];

        return [
            "output" => $last
        ];
    }
}
