<?php

namespace App\Console\Commands;

use App\AI\CohereAIGateway;
use App\Models\Plugin;
use GuzzleHttp\Client;
use Illuminate\Console\Command;

class GeneratePluginShortDescriptions extends Command
{
    protected $signature = 'plugins:short_desc';

    protected $description = 'Generate plugin short descriptions';

    protected Client $httpClient;

    public function __construct(Client $httpClient)
    {
        parent::__construct();
        $this->httpClient = $httpClient;
    }

    public function handle()
    {
        $this->info('Generating plugins short desc...');

        $plugins = Plugin::where('short_description', '')
            ->orWhereNull('short_description')
            ->take(50)->get();

        $gateway = new CohereAIGateway($this->httpClient);

        $saveCount = 0;

        foreach ($plugins as $plugin) {
            $summary = null;
            // Plugin short descriptions are not extremely important, so
            // if there is an error or the summary is empty we just
            // use a truncated version of the original description.
            // This prevent wasting calls to the AI service for unexpected issues.
            try {
                $summary = $this->summarizeDescription($plugin, $gateway);
            } catch (\Exception $e) {
                $this->error("An exception occurred while summarizing plugin {$plugin->id}: {$e->getMessage()}");
            }

            if (! $summary || empty($summary)) {
                $summary = $plugin->getShortDescriptionAttribute();
                $this->error("Failed to summarize plugin {$plugin->id}");
            }

            // Update the plugin short description
            $plugin->short_description = $summary;
            $plugin->save();
            $saveCount++;
            $this->info("Plugin desc {$plugin->id} summarized: $summary");

            // wait for half a second
            usleep(500000);
        }

        // log how many we updated
        $this->info("Updated {$saveCount} plugins");
    }

    protected function summarizeDescription($plugin, $gateway)
    {
        $min = 16;
        $max = 64;
        $desc = $plugin->description;
        // if already short, return without summarizing
        if (! $desc || empty($desc) || str_word_count($desc) < $min) {
            return $desc ?? '';
        }
        $summary = $gateway->summarize($desc, $max, "Create a short $max-word summary that explains this plugin behavior, keep it impersonal and concise.");

        // Ensure the summary is less than 8 words
        if ($summary) {
            $words = explode(' ', $summary);
            if (count($words) > $max) {
                $summary = implode(' ', array_slice($words, $min, $max)).'...';
            }
        }

        return $summary;
    }
}
