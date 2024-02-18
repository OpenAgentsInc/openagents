<?php

namespace App\Models;

use App\Traits\StepActions;
use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;
use GuzzleHttp\Psr7\Request;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use OpenAI;

class StepExecuted extends Model
{
    use HasFactory, StepActions;

    protected $guarded = [];

    public function llmInference($input, Conversation $conversation, $streamFunction)
    {
        $messages = [
            [
                "role" => "system",
                "content" => "You are a helpful AI agent named " . $conversation->agent->name . ". Your description is " . $conversation->agent->description
            ],
        ];

        $previousMessages = Message::where('conversation_id', $conversation->id)
            ->orderBy('created_at', 'asc')
            ->take(15)
            ->get()
            ->toArray();

        // Add previous messages to the array
        foreach ($previousMessages as $msg) {
            $messages[] = [
                "role" => $msg['sender'] === 'user' ? 'user' : 'assistant',
                "content" => $msg['body']
            ];
        }

        // Add the current user input as the last element
        $messages[] = ["role" => "user", "content" => $input['input']];

        $client = OpenAI::client(env("OPENAI_API_KEY"));
        $stream = $client->chat()->createStreamed([
            'model' => 'gpt-4',
            'messages' => $messages,
            'max_tokens' => 6024,
        ]);

        foreach($stream as $response) {
            // $response->choices[0]->toArray();
            $streamFunction($response);
        }
    }

    public function old_llmInference($input)
    {
        $inputString = $input['input'];
        $client = new Client();

        $url = 'https://api.openai.com/v1/chat/completions';
        // $url = 'https://api.together.xyz/inference';
        $model = 'gpt-4';
        // $model = 'DiscoResearch/DiscoLM-mixtral-8x7b-v2';
        $messages = [
                [
                    "role" => "system",
                    "content" => "You are a helpful agent on OpenAgents.com. Respond to the user concisely.",
                ],
                [
                    "role" => "user",
                    "content" => $inputString,
                ]
            ];
        $data = [
            "model" => $model,
            "messages" => $messages,
            "max_tokens" => 1024,
            "temperature" => 0.7,
            // "stream" => true
            // "stream_tokens" => true
        ];
        try {
            $response = $client->post($url, [
                'json' => $data,
                'stream' => true,
                'headers' => [
                    // 'Authorization' => 'Bearer ' . env('TOGETHER_API_KEY'),
                    'Authorization' => 'Bearer ' . env('OPENAI_API_KEY'),
                ],
            ]);
            $content = '';
            $stream = $response->getBody();
            while (!$stream->eof()) {
                $line = $stream->readLine();
                if ($line) {
                    $responseLine = json_decode($line, true);
                    // Process each line as needed
                    // For example, you might want to concatenate the content from each "delta" in choices
                    if (isset($responseLine["choices"][0]["delta"]["content"])) {
                        $content .= $responseLine["choices"][0]["delta"]["content"];
                    }
                }
            }
            // foreach ($this->readStream($stream) as $responseLine) {
            //     dd($responseLine);
            //     $token = $responseLine["choices"][0]["text"];
            //     $content .= $token;
            //     dd($token);
            // }
        } catch (RequestException $e) {
            $content = $e->getMessage();
            dd($content);
        }
    }

    public function run(Conversation $conversation, callable $streamFunction = null)
    {
        $input = (array) json_decode($this->input);

        // If the StepExecuted's step name is LLM Inference, override with our own streaming one
        if ($this->step->name === 'LLM Inference') {
            return $this->llmInference($input, $conversation, $streamFunction);
        }

        // Based on the category, run the appropriate StepAction. [validation, embedding, similarity_search, inference]
        $category = $this->step->category;

        // If category is inference, set the current conversation so inference StepAction can access it
        // if ($category === 'inference') {
        //     $this->setConversation($this->task_executed->conversation);
        // }

        $output = $this->$category($input, $this->task_executed->conversation);
        // Update the StepExecuted with completed status and output
        $this->update([
            'status' => 'completed',
            'output' => $output
        ]);
        return $output;
    }

    public function step()
    {
        return $this->belongsTo(Step::class);
    }

    public function task_executed()
    {
        return $this->belongsTo(TaskExecuted::class);
    }

    public function user()
    {
        return $this->belongsTo(User::class);
    }
}
