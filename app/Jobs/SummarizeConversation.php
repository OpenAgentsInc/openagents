<?php

namespace App\Jobs;

use App\Models\Conversation;
use App\Services\AIGateways\GPUtopiaGateway;
use App\Services\AIGateways\OpenAIGateway;
use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Queue\SerializesModels;
use Exception;

class SummarizeConversation implements ShouldQueue
{
    use Dispatchable, InteractsWithQueue, Queueable, SerializesModels;

    protected $conversation;

    /**
     * Create a new job instance.
     *
     * @param  Conversation  $conversation
     */
    public function __construct(Conversation $conversation)
    {
        $this->conversation = $conversation;
    }

    /**
     * Execute the job.
     */
    public function handle(): void
    {
        try {
            $summary = $this->summarizeWithGateway(new GPUtopiaGateway(), $this->conversation);
        } catch (Exception $e) {
            // If GPUtopia fails, fall back to OpenAI
            report($e); // Optional: Report or log the exception
            $summary = $this->summarizeWithGateway(new OpenAIGateway(), $this->conversation);
        }

        // Example: Save the summary to the conversation
        // Ensure your Conversation model has a 'summary' field or similar
        $this->conversation->summary = $summary['output'];
        $this->conversation->save();
    }

    /**
     * Attempt to summarize the conversation with a specified AI gateway.
     *
     * @param  mixed  $gateway
     * @param  Conversation  $conversation
     * @return array
     */
    protected function summarizeWithGateway($gateway, Conversation $conversation): array
    {
        // Compile conversation messages into a single text for summarization
        $text = $conversation->messages->pluck('content')->join(' ');
        // Call the inference method on the gateway, adjust as needed for your gateway's API
        return $gateway->inference($text);
    }
}
