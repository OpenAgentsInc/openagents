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

    public function getUserConversation()
    {
        $convo = $this->conversations()->where('user_id', auth()->id())->first();

        if (!$convo) {
            $convo = Conversation::create([
                'user_id' => auth()->id(),
                'agent_id' => $this->id,
            ]);
        }

        return $convo;
    }

    public function run($input)
    {
        // Get the first task
        $task = $this->tasks()->first()->load('steps');

        // Create from it a TaskExecuted
        $task_executed = TaskExecuted::create([
            'task_id' => $task->id,
            // Current user ID if authed or null
            'user_id' => auth()->id(),
            'status' => 'pending'
        ]);

        // Loop through all the task's steps, passing the output of each to the next
        foreach ($task->steps as $step) {
            if ($step->order !== 1) {
                $input = $prev_step_executed->output;
            }
            // Create a new StepExecuted with this step and task_executed
            $step_executed = StepExecuted::create([
                'step_id' => $step->id,
                'input' => json_encode($input),
                'order' => $step->order,
                'task_executed_id' => $task_executed->id,
                'user_id' => auth()->id(),
                'status' => 'pending',
            ]);
            $step_executed->output = $step_executed->run();
            $step_executed->save();

            $prev_step_executed = $step_executed;
        }

        // Return the output of the final StepExecuted
        return $step_executed->fresh()->output;
    }

    public function brains()
    {
        return $this->hasMany(Brain::class);
    }

    public function brain()
    {
        return $this->brains()->first();
    }

    public function conversations()
    {
        return $this->hasMany(Conversation::class);
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
            'body' => $body
        ]);
    }

    public function reflect()
    {
        try {
            // Grab all the steps for this agent
            $steps = $this->steps()->get();

            $thoughts = [];

            // Loop through each step and create a Thought for each
            foreach ($steps as $step) {
                $messages = [
                    ['role' => 'system', 'content' => "You are an AI agent specializing in understanding unstructured data."],
                    ['role' => 'user', 'content' => "What do you notice about this data?: " . json_encode($step)],
                ];
                $input = [
                    'model' => "gpt-4",
                    'messages' => $messages,
                ];

                $gateway = new OpenAIGateway();
                $response = $gateway->makeChatCompletion($input);

                $output = $response['choices'][0];
                $comment = $output['message']['content'];

                $thought = Thought::create([
                    'agent_id' => $this->id,
                    'body' => $comment,
                    // 'body' => "I notice that I have {$steps->count()} steps."
                ]);

                $thoughts[] = $thought->body;
            }

            return $thoughts;
        } catch (\Exception $e) {
            // If $thoughts count is greater than 0, return the thoughts - otherwise return the error message
            if (count($thoughts) > 0) {
                return $thoughts;
            }

            return $e->getMessage();
        }
    }
}
