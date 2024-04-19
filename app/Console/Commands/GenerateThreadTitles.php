<?php

namespace App\Console\Commands;

use App\AI\CohereAIGateway;
use App\Models\Thread;
use GuzzleHttp\Client;
use Illuminate\Console\Command;

class GenerateThreadTitles extends Command
{
    protected $signature = 'threads:title';

    protected $description = 'Generate thread titles';

    public function handle()
    {
        $this->info('Generating thread titles...');
        $threads = Thread::where('title', 'New chat')
            ->has('messages', '>', 1)
            ->latest()
            ->take(100)
            ->get();

        $httpClient = new Client();
        $gateway = new CohereAIGateway($httpClient);

        foreach ($threads as $thread) {
            $summary = $this->summarizeConversation($thread, $gateway);

            // Update the thread title with the summary
            if ($summary) {
                $thread->title = $summary;
                $thread->save();
                $this->info("Thread {$thread->id} summarized: $summary");
            } else {
                $this->error("Failed to summarize Thread {$thread->id}");
            }

            sleep(0.5);
        }
    }

    protected function summarizeConversation($thread, $gateway)
    {
        $conversationText = $thread->messages->pluck('body')->implode(' ');
        $summary = $gateway->summarize($conversationText);

        // Ensure the summary is less than 8 words
        if ($summary) {
            $words = explode(' ', $summary);
            if (count($words) > 7) {
                $summary = implode(' ', array_slice($words, 0, 7));
            }
        }

        return $summary;
    }
}
