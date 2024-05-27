<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;
use Illuminate\Support\Facades\File;
use Spatie\DbDumper\Databases\MySql;

class DumpRelevantTables extends Command
{
    protected $signature = 'dump';

    protected $description = 'Dump the tables we want to keep';

    public function handle()
    {
        // Get this stuff from env()
        $databaseName = env('DB_DATABASE');
        $userName = env('DB_USERNAME');
        $password = env('DB_PASSWORD');

        // Get a string timestamp for the dumpToFile
        $timestamp = now()->format('Y-m-d_H-i-s');

        // Define the dump path within the storage directory
        $dumpPath = storage_path("app/dumps/{$databaseName}_{$timestamp}.sql");

        // Ensure the directory exists
        File::ensureDirectoryExists(dirname($dumpPath));

        // Perform the dump
        MySql::create()
            ->setDbName($databaseName)
            ->setUserName($userName)
            ->setPassword($password)
            ->includeTables([
                'agent_files',
                'agents',
                'messages',
                'nostr_accounts',
                'nostr_jobs',
                'sessions',
                'subscription_items',
                'subscriptions',
                'threads',
                'users',
            ])
            ->dumpToFile($dumpPath);

        $this->info("Database dump created successfully: $dumpPath");
    }
}
