<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;
use RecursiveDirectoryIterator;
use RecursiveIteratorIterator;

class UploadGeminiFiles extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'gemini:folder';

    /**
     * The console command description.
     *
     * @var string
     */
    protected $description = 'Uploads every file in the MMVP directory';

    /**
     * Execute the console command.
     */
    public function handle()
    {
        $directory = base_path('resources/localimages/MMVP');

        $rii = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($directory));

        foreach ($rii as $file) {
            if ($file->isDir()) {
                continue;
            }
            $filePath = $file->getPathname();

            $this->info('Uploading: '.$filePath);

            // Call the other Artisan command for each file
            $this->call('gemini:upload', [
                'filePath' => $filePath,
            ]);
        }

        $this->info('All files have been processed.');
    }
}
