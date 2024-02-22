<?php

namespace App\Models;

use App\Services\OpenAIGateway;
use App\Traits\UsesChat;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Agent extends Model
{
    use HasFactory, UsesChat;

    protected $guarded = [];

    public function runTask($input, Task $task, Conversation $conversation, $logFunction, $streamFunction)
    {

    }


    public function run($input, $task = null, $logFunction = null, $streamFunction = null)
    {

        $conversation = $this->getUserConversation();

        // Append the input to the conversation
        $conversation->messages()->create([
            'user_id' => auth()->id(),
            'body' => $input['input'],
            'sender' => 'user',
        ]);

        $userInput = $input;
        if (! $task) {
            // If no provided task, get the first task
            $task = $this->tasks()->first()->load('steps');
        }

        // Create from it a TaskExecuted
        $task_executed = TaskExecuted::create([
            'task_id' => $task->id,
            // Current user ID if authed or null
            'user_id' => auth()->id(),
            'status' => 'pending',
        ]);

        foreach ($task->steps as $step) {
            // Log the step name using the provided logging function
            if ($logFunction) {
                $logFunction($step->name);
            }
            if ($step->order !== 1) {
                $input = $prev_step_executed->output;
            }

            if ($step->name === 'LLM Inference') {
                $input = json_encode([
                    'input' => "Respond to this user input with the following context: \n User Input: {$userInput['input']} \n\n Context: {$input}",
                ]);
            }

            // if step category is "plugin", augment the input with params
            if ($step->category === 'plugin') {
                $params = json_decode($step->params);

                // If the previous step executed is the first step, use the input directly
                // Otherwise, use the output of the previous step executed
                if ($step->order === 1) {
                    $input = json_encode([
                        'input' => $input,
                        'plugin_id' => $params->plugin_id,
                        'function' => $params->function,
                    ]);
                } elseif ($step->order === 2) {

                    // the previous step output will either be an array with a key of output or just a string, so handle both cases
                    if (is_array($prev_step_executed->output)) {
                        $input = json_encode([
                            'input' => $prev_step_executed->output['output'], // Assign the output directly
                            'plugin_id' => $params->plugin_id,
                            'function' => $params->function,
                        ]);
                    } else {
                        // temporarily take this array of strings - and json decode and grab the first element and pass that to the input array as ['url' => {first element}]
                        $temp = json_decode($prev_step_executed->output);
                        // dd($temp); // "["https://raw.githubusercontent.com/OpenAgentsInc/plugin-url-scraper/main/src/lib.rs"]"
                        $temp = json_decode($temp);

                        $input = json_encode([
                            'input' => [
                                'url' => $temp[0], // Assign the output directly
                            ],
                            'plugin_id' => $params->plugin_id,
                            'function' => $params->function,
                        ]);
                    }
                }
            } elseif ($step->category === 'L402') {
                $params = json_decode($step->params);
                $input = json_encode([
                    // 'input' => $prev_step_executed->output, // Assign the output directly
                    'url' => $params->url,
                ]);
            } else {
                $input = json_encode($input);
            }

            // Create a new StepExecuted with this step and task_executed
            $step_executed = StepExecuted::create([
                'step_id' => $step->id,
                'input' => $input,
                'order' => $step->order,
                'task_executed_id' => $task_executed->id,
                'user_id' => auth()->id(),
                'status' => 'pending',
            ]);
            $step_executed->output = $step_executed->run($conversation, $streamFunction);
            $step_executed->save();

            $prev_step_executed = $step_executed;
        }

        $lastoutput = $step_executed->fresh()->output;

        // We get output as json - convert to array
        $output = json_decode($lastoutput, true);

        // Append the input to the conversation
        $conversation->messages()->create([
            'user_id' => auth()->id(),
            'body' => $output['output'],
            'sender' => 'agent',
        ]);

        // Return the output of the final StepExecuted
        return $step_executed->fresh()->output;
    }

    public function conversations()
    {
        return $this->hasMany(Conversation::class);
    }

    public function files()
    {
        return $this->hasMany(File::class);
    }

    public function steps()
    {
        return $this->hasMany(Step::class);
    }

    public function tasks()
    {
        return $this->hasMany(Task::class);
    }

    public function thoughts()
    {
        return $this->hasMany(Thought::class);
    }

    public function user()
    {
        return $this->belongsTo(User::class);
    }

    public function sendMessage($conversationId, $body)
    {
        Message::create([
            'sender' => 'agent',
            'conversation_id' => $conversationId,
            'user_id' => $this->user->id,
            'body' => $body,
        ]);
    }
}
