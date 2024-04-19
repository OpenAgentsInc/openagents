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

        // for now we gotta unfuck previous shit titles, so get threads where last updated more than 15 minutes ago
        $threads = Thread::where('updated_at', '<', now()->subMinutes(15))
            ->has('messages', '>', 1)
            ->latest()
            ->take(50)
            ->get();

        $httpClient = new Client();
        $gateway = new CohereAIGateway($httpClient);

        $saveCount = 0;

        foreach ($threads as $thread) {
            $summary = $this->summarizeConversation($thread, $gateway);

            // Update the thread title with the summary
            if ($summary) {
                $thread->title = $summary;
                $thread->save();
                $saveCount++;
                $this->info("Thread {$thread->id} summarized: $summary");
            } else {
                $this->error("Failed to summarize Thread {$thread->id}");
            }

            sleep(0.5);
        }

        // log how many we updated
        $this->info("Updated {$saveCount} conversations");
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
