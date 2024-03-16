<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;

class BenchTest extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'bench';

    /**
     * The console command description.
     *
     * @var string
     */
    protected $description = 'Command description';

    /**
     * Execute the console command.
     */
    public function handle()
    {
        // Load the /resources/swebench/swe-bench-dev.json file and decode it
        $benchData = json_decode(file_get_contents(base_path('resources/swebench/swe-bench-dev.json')), true);
        dump($benchData);
    }
}
