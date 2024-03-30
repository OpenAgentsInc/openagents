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
    protected $signature = 'gemini:upload {filePath : The path to the file you want to upload}';

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
        // Retrieve the file path argument
        $filePath = $this->argument('filePath');
        // Extract the filename from the path
        $filename = pathinfo($filePath, PATHINFO_BASENAME);
        // Escaping the file path and display name for shell command
        $escapedFilePath = escapeshellarg($filePath);
        $displayName = escapeshellarg($filename);

        $scriptPath = base_path('scripts/gemini-upload-file.sh'); // Adjust this path to where your script is located.

        $command = "{$scriptPath} -a {$apiKey} -i {$escapedFilePath} -d {$displayName}";

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
