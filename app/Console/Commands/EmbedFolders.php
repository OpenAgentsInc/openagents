<?php

namespace App\Console\Commands;

use App\Services\Embedder;
use Illuminate\Console\Command;

class EmbedFolders extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'embed';

    /**
     * The console command description.
     *
     * @var string
     */
    protected $description = 'Create vector embeddings from folders';

    /**
     * Execute the console command.
     */
    public function handle()
    {
        $folders = [
          'app/Http/Controllers'
          // 'app/Models',
          // 'database/migrations',
          // 'tests/Unit'
        ];

        $embedder = new Embedder();

        $this->info('Embedding folders...');
        foreach ($folders as $folder) {
          $this->info('Embedding ' . $folder);
          $embedder->createEmbeddingsForFolder($folder);
        }

        $this->info('Done!');
    }
}
