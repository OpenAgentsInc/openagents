<?php

namespace App\Traits;

use App\Http\Controllers\StreamController;
use App\Models\Plugin;
use App\Services\Embedder;
use App\Services\GitHub;
use App\Services\L402 as L402Service;
use Illuminate\Support\Facades\Http;

trait StepActions
{
    public function code_analysis($input)
    {
        $url = $input['url'];
        $github = new GitHub($url);

        //      $readme = explode('## Video series', $github->getReadme())[0];
        $migrations = $github->fetchFileContents('database/migrations/');

        // $migrations is an array of paths. Let's loop through and fetch the contents of the first two migrations.
        $migration1 = $github->fetchFileContents($migrations[7]);
        $migration2 = $github->fetchFileContents($migrations[8]);
        $migration3 = $github->fetchFileContents($migrations[9]);
        //      $migration4 = $github->fetchFileContents($migrations[3]);

        $prompt = 'Analyze the given files, specifically whether there are any syntax mismatches in the migrations.';

        // Compile one context string with all the information and a bit of explanation - from readme, routes, and composer:
        $context = 'Use this context:'."\n---\n";
        //        $context .= "This is the README.md file from the repository:\n".$url."\n---\n";
        //       $context .= $readme."\n---\n";
        $context .= "These are the migrations from the repository:\n".$url."\n---\n";
        $context .= $migration1."\n---\n";
        $context .= $migration2."\n---\n";
        $context .= $migration3."\n---\n";
        //       $context .= $migration4."\n---\n";
        //        $context .= "This is the tailwind.config.js file from the repository:\n".$url."\n---\n";
        //       $context .= $tailwind."\n---\n";
        // $context .= "These are the routes from the repository:\n".$url."\n---\n";
        // $context .= $routes."\n---\n";
        // $context .= "This is the composer.json file from the repository:\n".$url."\n---\n";
        // $context .= $composer."\n---\n";

        $analysis = $this->analyze($context, $prompt);

        return [
            'input' => $url,
            'analysis' => $analysis,
        ];
    }

    public function analyze(string $context, string $prompt)
    {
        // Truncate or summarize the repositoryHierarchy to fit within the max context length of 2048 characters
        $truncatedHierarchy = $this->truncateOrSummarize($context, 1800);

        try {
            $url = 'https://api.together.xyz/inference';
            // $model = 'codellama/CodeLlama-34b-Instruct-hf';
            $model = 'codellama/CodeLlama-70b-Instruct-hf';

            dump($context);
            dump("\n----\n");

            $data = [
                'model' => $model,
                'messages' => [
                    [
                        'role' => 'system',
                        'content' => $prompt,
                    ],
                    [
                        'role' => 'user',
                        'content' => $context,
                    ],
                ],
                'max_tokens' => 824,
                'temperature' => 0.7,
            ];

            $response = Http::withHeaders([
                'Authorization' => 'Bearer '.env('TOGETHER_API_KEY'),
            ])->post($url, $data);

            if ($response->successful()) {
                $body = $response->json();
                if ($body['status'] === 'finished') {
                    $last = $body['output']['choices'][0]['text'];
                } else {
                    $last = 'Error: '.$body['status'];
                }
            } else {
                $last = 'Error: '.$response->status();
            }

            return $last;
        } catch (\Exception $e) {
            // Handle exception or errors here
            echo $e->getMessage();
        }
    }

    /**
     * Truncate or summarize the input string to fit within a maximum character length.
     *
     * @param  string  $input
     * @param  int  $maxLength
     * @return string
     */
    private function truncateOrSummarize($input, $maxLength)
    {
        if (strlen($input) > $maxLength) {
            // Truncate or implement a summarization method
            return substr($input, 0, $maxLength); // Example: simple truncation
        }

        return $input;
    }

    public function L402($input)
    {
        $url = $input['url'];

        // Create a new L402 service object
        $l402Service = new L402Service();

        // Handle the L402 request
        $response = $l402Service->handleL402Request($url);

        // Process the response
        if ($response->successful()) {
            // Successful response processing
            $body = $response->body();

            // Attempt to decode the response body as JSON
            $decodedBody = json_decode($body, true);

            // Check if json_decode was successful (i.e., the body was valid JSON)
            if (json_last_error() === JSON_ERROR_NONE) {
                // The body was valid JSON, process the decoded array as needed
                return $decodedBody;
            } else {
                // The body was not valid JSON (e.g., plain text), process the string as needed
                return json_encode(['output' => $body]);
            }
        } else {
            // Handle error scenarios
            throw new \Exception('Error accessing the API: '.$response->status());
        }
    }

    public function plugin($input)
    {
        $input = $this->validatePlugin($input);
        $plugin_id = $input['plugin_id'];
        $plugin = Plugin::findOrFail($plugin_id);

        // if $input["input"] is not an array, convert it to an array
        // if (! is_array($input['input'])) {
        //     $input['input'] = (array) $input['input'];
        // }
        //

        $actualInput = $input;

        // if input['input'] is a stdclass, convert it to an array
        if (is_object($input['input'])) {
            $input['input'] = (array) $input['input'];
        }

        // if $input['input'] is an array with key url, then that is the input
        if (array_key_exists('url', (array) $input['input'])) {
            $actualInput = $input['input'];
            // dd($actualInput);
        } elseif (! is_string($input['input'])) {         // if $input["input"] is not a string, convert it to a string
            // dd($input['input']);
            // $input['input'] = $input['input']->input;
            $actualInput = $input['input']['input'];
        }

        // if $actualInput is not a string, dd it
        if (! is_string($actualInput)) {
            $actualInput = json_encode($actualInput);
        }

        // dd([
        //     'plugin_id' => $plugin_id,
        //     'input' => $actualInput,
        //     'function' => $input['function'],
        // ]);
        //
        if ($input['function'] == 'inference') {
            // dd($input);
            // $actualInput = json_encode([
            //     'model_name' => 'gpt-4',
            //     'input_content' => 'Speculate about this:'.$input,
            //     'api_key' => env('OPENAI_API_KEY'),
            // ]);
            $actualInput = $input['input'];
        }
        $pluginOutput = $plugin->call($input['function'], $actualInput);

        return json_encode($pluginOutput);
    }

    public function validation($input, $conversation)
    {
        // dd($input);

        // convert stdclass $input to an array
        $output = (array) $input;

        return $output;

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

    public function embedding($input, $conversation)
    {
        $input = $input['input'];
        // Check if input is a string
        if (! is_string($input)) {
            echo "Embedding input is not a string.\n";
            dd($input);
        }

        // If we're in a test environment, fake this
        if (env('APP_ENV') == 'testing') {
            return [
                'input' => $input,
                'embedding' => [], // Embedder::createFakeEmbedding(),
            ];
        }

        dd('not implemented');

        $gateway = new QueenbeeGateway();
        $result = $gateway->createEmbedding($input);
        $embedding = $result[0]['embedding'];

        return [
            'input' => $input,
            'embedding' => $embedding,
        ];
    }

    public function similarity_search($input, $conversation, $take = 8)
    {
        return [
            'input' => [0, 0],
            'context' => [
                'do this via a plugin',
            ],
        ];
    }

    public function inference($input, $conversation)
    {
        // If $input["context"] does not exist, set it to an empty array
        if (! array_key_exists('context', $input)) {
            $input['context'] = [];
        }

        // Expect input to be an array of strings with relevant context

        // Check if input is an array
        if (! is_array($input) || ! is_array($input['context'])) {
            echo "Input is not an array.\n";
            dd($input);
        }

        // If it is an array, check if it is an array of strings
        foreach ($input['context'] as $key => $value) {
            if (! is_string($value)) {
                echo "Input is not an array of strings.\n";
                dd($input);
            }
        }

        // If it is an array of strings, then we can proceed with inference
        // Loop through them and concatenatte them into a single string with line breaks with an introductory explanation
        $context = 'Use this context:'."\n---\n";
        foreach ($input['context'] as $key => $value) {
            $context .= $value."\n---\n";
        }
        $context .= '---';

        // $gateway = new OpenAIGateway();

        // If we're in a test environment, fake this
        if (env('APP_ENV') == 'testing') {
            $last = 'This is a test response.';

            if ($conversation) {
                $conversation->messages()->create([
                    'user_id' => auth()->id() ?? null,
                    'body' => $last,
                    'sender' => 'agent',
                ]);
            }
        } else {
            // Initiate new StreamController
            $streamer = new StreamController();
            $last = $streamer->doChat($input['input'], $conversation, $context);
            // if ($this->conversation) {
            //     $last = $streamer->doChat($input["input"], null, $context);
            // } else {
            //     $last = $streamer->doChat($input["input"], $this->conversation, $context);
            // }
        }

        // Save message to conversation

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
            'output' => $last,
        ];
    }

    // expect an array with plugin_id and input, throw error otherwise.
    private function validatePlugin($input)
    {
        if (! is_array($input)) {
            echo "Input is not an array.\n";
            dd($input);
        }

        if (count($input) !== 3) {
            // echo "Input does not have three keys. hardcoding ...\n";
            $theput = json_encode([
                'model_name' => 'gpt-4',
                'input_content' => $input['input'],
                'api_key' => env('OPENAI_API_KEY'),
            ]);

            return [
                'plugin_id' => 3,
                'input' => $theput,
                'function' => 'inference',
            ];

        }

        if (! array_key_exists('plugin_id', $input)) {
            echo "Input does not have key plugin_id.\n";
            dd($input);
        }

        if (! array_key_exists('input', $input)) {
            echo "Input does not have key input.\n";
            dd($input);
        }

        if (! array_key_exists('function', $input)) {
            echo "Input does not have key function.\n";
            dd($input);
        }

        return $input;
    }
}
