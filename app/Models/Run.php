<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class Run extends Model
{
    use HasFactory;

    protected $guarded = [];

    // belongs to a flow
    public function flow(): BelongsTo
    {
        return $this->belongsTo(Flow::class);
    }

    /**
     * Triggers the execution of the associated flow.
     *
     * @param  callable  $streamingFunction  A callback function for streaming the output.
     * @return mixed The result of the flow execution.
     */
    public function trigger($streamingFunction)
    {
        // Ensure the flow is loaded
        $flow = $this->load('flow');

        // Get the nodes of the flow
        $nodes = $flow->nodes()->orderBy('sequence', 'asc')->get(); // Assuming you have a 'sequence' field to order nodes

        // Execute each node in sequence
        foreach ($nodes as $node) {
            // Here, you'd call a service or logic that knows how to execute the node.
            // For demonstration, let's assume you have a NodeExecutionService that handles the logic.
            $nodeExecutionService = new NodeExecutionService();
            $result = $nodeExecutionService->executeNode($node, $this->input, $streamingFunction);

            // If your node execution can directly affect the flow (like causing it to stop early),
            // you might want to handle that logic here.
        }

        // Return the final result
        // This is a simplified view. You might need to collect and aggregate results from each node.
        return $result ?? 'Flow completed without output.';
    }
}
