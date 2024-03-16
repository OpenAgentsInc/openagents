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
        $benchData = json_decode(file_get_contents(base_path('storage/benchmarks/swe-bench.json')), true);

        // Loop through each benchmark
        foreach ($benchData as $benchmark) {
            // keys are repo, instance_id, base_commit, patch, problem_statement, hints_text, created_at, version, FAIL_TO_PASS, PASS_TO_PASS, environment_setup_commit

            // Enter a new directory and git clone this repo
            $repo = $benchmark['repo'];
            $instanceId = $benchmark['instance_id'];
            $baseCommit = $benchmark['base_commit'];
            //            $patch = $benchmark['patch'];
            $problemStatement = $benchmark['problem_statement'];

            // Create a new directory for the benchmark
            $benchmarkDir = storage_path("benchmarks/$instanceId");
            if (! file_exists($benchmarkDir)) {
                mkdir($benchmarkDir, 0777, true); // Enable recursive directory creation
            }

            // Change to the benchmark directory
            chdir($benchmarkDir);

            // Clone the repository - but only if it doesn't exist
            if (! file_exists("$benchmarkDir/.git")) {
                exec("git clone https://github.com/$repo .");
            }

            // Checkout the base commit
            exec("git checkout $baseCommit");

            dd($problemStatement);
        }
    }
}
