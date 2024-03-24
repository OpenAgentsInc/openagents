<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;
use Illuminate\Support\Facades\Storage;

class LogFilepaths extends Command
{
    // The name and signature of the console command.
    protected $signature = 'log:filepaths';

    // The console command description.
    protected $description = 'Logs all filepaths in the specified storage folder';

    // Create a new command instance.
    public function __construct()
    {
        parent::__construct();
    }

    // Execute the console command.
    public function handle()
    {
        $folderPath = 'git-repo'; // Relative to Laravel's storage folder
        $files = Storage::allFiles($folderPath);

        $this->line(count($files).' files found.');

        foreach ($files as $file) {
            // Log each file's relative path - but first remove the $folderPath from the beginning
            $this->line($file = str_replace($folderPath.'/', '', $file));

            // Skip if path includes ".git"
            if (strpos($file, '.git') !== false) {
                continue;
            }

            // $this->line(Storage::get($file));
        }

        $this->info('All file paths have been logged.');
    }
}
