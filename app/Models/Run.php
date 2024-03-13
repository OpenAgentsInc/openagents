<?php

namespace App\Models;

use App\AI\MistralAIGateway;
use App\Services\SemanticRouter;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class Run extends Model
{
    use HasFactory;

    protected $fillable = ['agent_id', 'flow_id', 'thread_id', 'input'];

    // belongs to an Agent
    public function agent(): BelongsTo
    {
        return $this->belongsTo(Agent::class);
    }

    // belongs to a Flow
    public function flow(): BelongsTo
    {
        return $this->belongsTo(Flow::class);
    }

    // belongs to a Thread
    public function thread(): BelongsTo
    {
        return $this->belongsTo(Thread::class);
    }

    /**
     * Triggers the execution of the associated flow.
     *
     * @param  callable  $streamingFunction  A callback function for streaming the output.
     * @return string The aggregated result of the flow execution.
     */
    public function trigger(callable $streamingFunction): string
    {
        // Ensure the flow is loaded along with its nodes
        $this->load('flow.nodes');

        $input = $this->input; // Assume this run model has an 'input' attribute.
        $output = '';

        // First see if we should override node with a semantic route
        // Vectorize the input
        $gateway = new MistralAIGateway();
        $vectorizedInput = $gateway->embed($input);

        $router = new SemanticRouter();
        $route = $router->route($vectorizedInput);

        dd($route);

        // Execute each node in sequence (assuming nodes are already sorted by their execution order)
        foreach ($this->flow->nodes as $node) {

            // Call the node's trigger method and pass the current input
            $nodeOutput = $node->trigger([
                'agent' => $this->agent,
                'flow' => $this->flow,
                'thread' => $this->thread,
                'input' => $input,
                'streamingFunction' => $streamingFunction,
            ]);

            // Set the output of the current node as the input for the next node
            $input = $nodeOutput;

            // Append the output to the overall run output (this can be adjusted based on your needs)
            $output .= $nodeOutput;
        }

        // Return the final aggregated output
        return $output;
    }
}
