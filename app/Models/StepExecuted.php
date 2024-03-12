<?php

namespace App\Models;

use App\Services\Inferencer;
use App\Traits\StepActions;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class StepExecuted extends Model
{
    use HasFactory, StepActions;

    protected $guarded = [];

    public function run(Conversation $conversation, ?callable $streamFunction = null)
    {
        $input = (array) json_decode($this->input);

        // If the StepExecuted's step name is LLM Inference, override with our own streaming one
        if ($this->step->name === 'LLM Inference') {
            return Inferencer::llmInference($input, $conversation, $streamFunction);
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
            'output' => $output,
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
