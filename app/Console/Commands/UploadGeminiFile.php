<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;

class UploadGeminiFile extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'gemini:upload';

    /**
     * The console command description.
     *
     * @var string
     */
    protected $description = 'Upload a file via Gemini File API';

    /**
     * Execute the console command.
     */
    public function handle()
    {
        $apiKey = escapeshellarg(env('GEMINI_API_KEY'));
        $filePath = escapeshellarg(base_path('resources/localimages/home.png'));
        $displayName = escapeshellarg('Demo Home Screenshot');

        $scriptPath = base_path('scripts/gemini-upload-file.sh'); // Adjust this path to where your script is located.

        $command = "{$scriptPath} -a {$apiKey} -i {$filePath} -d {$displayName}";

        $output = null;
        $returnVar = null;
        exec($command, $output, $returnVar);

        if ($returnVar === 0) {
            $this->info('Image uploaded successfully.');
        } else {
            $this->error('Failed to upload image.');
            foreach ($output as $line) {
                $this->error($line);
            }
        }
    }
}
