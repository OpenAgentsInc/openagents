<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;
use Symfony\Component\Process\Exception\ProcessFailedException;
use Symfony\Component\Process\Process;

class GitCloneCommand extends Command
{
    // The name and signature of the console command.
    protected $signature = 'git:clone {repoUrl}';

    // The console command description.
    protected $description = 'Clone a GitHub repository into a Laravel storage folder';

    // Execute the console command.
    public function handle()
    {
        $repoUrl = $this->argument('repoUrl');
        $destinationFolder = storage_path('app/git-repo'); // Hardcoded destination folder

        // Ensure the destination folder exists or create it
        if (! is_dir($destinationFolder)) {
            mkdir($destinationFolder, 0755, true);
        }

        $this->info("Cloning repository into {$destinationFolder}");

        // Define and execute the command
        $command = sprintf('git clone %s %s', escapeshellarg($repoUrl), escapeshellarg($destinationFolder));
        $process = Process::fromShellCommandline($command);
        $process->run();

        // Executes after the command finishes
        if (! $process->isSuccessful()) {
            throw new ProcessFailedException($process);
        }

        $this->info($process->getOutput());
    }
}
