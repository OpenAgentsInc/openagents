<?php

namespace App\Console\Commands;

use App\AI\MistralAIGateway;
use Illuminate\Console\Command;

// Make sure to use the correct namespace for your MistralAIGateway class

class GenerateEmbedding extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'generate:embedding {text* : The text strings to generate embeddings for}';

    /**
     * The console command description.
     *
     * @var string
     */
    protected $description = 'Generates an embedding for the given text strings and saves it to a file';

    /**
     * Execute the console command.
     *
     * @return int
     */
    public function handle()
    {
        $texts = $this->argument('text');

        $gateway = new MistralAIGateway();
        $embedding = $gateway->embed($texts);

        if (isset($embedding['error'])) {
            $this->error('Error: '.$embedding['error']."\nDetails: ".json_encode($embedding['details']));

            return 1;
        } else {
            // Prepare the comment with input texts
            $comment = '// Input: '.implode(', ', $texts)."\n";
            $embeddingJson = json_encode($embedding);
            $filePath = base_path('embedding.txt');

            // Clear the file (if exists) and write the comment and embedding
            file_put_contents($filePath, $comment.$embeddingJson);
            $this->info("Embedding saved to: $filePath");

            return 0;
        }
    }
}
