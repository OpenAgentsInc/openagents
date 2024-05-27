<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;

class RestoreDatabase extends Command
{
    protected $signature = 'restore {dumpFile}';

    protected $description = 'Restore the database from a dump file';

    public function handle()
    {
        $dumpFile = $this->argument('dumpFile');

        // Check if the specified file exists
        if (! file_exists($dumpFile)) {
            $this->error("The dump file at {$dumpFile} does not exist.");

            return;
        }

        // Get this stuff from the environment
        $databaseName = env('DB_DATABASE');
        $userName = env('DB_USERNAME');
        $password = env('DB_PASSWORD');
        $host = env('DB_HOST');
        $port = env('DB_PORT', 3306);

        // Construct the command to restore the database
        $command = "mysql -h {$host} -P {$port} -u {$userName} --password={$password} {$databaseName} < {$dumpFile}";
        // info the command
        $this->info("Restoring database from {$dumpFile}...");
        $this->info($command);

        // Execute the command
        $result = null;
        $output = null;
        exec($command, $output, $result);

        if ($result === 0) {
            $this->info("Database successfully restored from {$dumpFile}");
        } else {
            $this->error('An error occurred while restoring the database. Please check the dump file and try again.');
        }
    }
}
