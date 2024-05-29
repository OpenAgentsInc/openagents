<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;
use Illuminate\Support\Facades\Artisan;

class DumpAndRestoreTables extends Command
{
    protected $signature = 'dumprestore';

    protected $description = 'Dump database, migrate and restore';

    public function handle()
    {
        // Step 1: Run the 'dump' command and capture the output
        $this->info('Running database dump...');
        Artisan::call('dump');

        // Get the output of the dump command
        $output = Artisan::output();
        $this->info($output);

        // Extract the dump file path from Artisan output
        preg_match('/Database dump copied to (\/home\/forge\/.+\.sql)/', $output, $matches);
        if (! isset($matches[1])) {
            $this->error('Could not determine the dump file path from the dump command output.');

            return;
        }
        $dumpFilePath = $matches[1];
        $this->info("Dump file path: $dumpFilePath");

        // Step 2: Wipe the database using db:wipe Artisan command
        $this->info('Wiping the database...');
        Artisan::call('db:wipe', ['--force' => true]);
        $this->info('Database wiped successfully');

        // Step 3: Run standard migrations using migrate Artisan command
        $this->info('Running migrations...');
        Artisan::call('migrate', ['--force' => true]);
        $this->info('Migrations completed successfully');

        // Step 4: Restore the database using the dump file path
        $this->info('Restoring the database...');
        Artisan::call('restore', ['dumpFile' => $dumpFilePath]);
        $restoreOutput = Artisan::output();
        $this->info($restoreOutput);
    }
}
