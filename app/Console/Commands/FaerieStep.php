<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;

class FaerieStep extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'faerie:step';

    /**
     * The console command description.
     *
     * @var string
     */
    protected $description = 'Makes Faerie advance a step';

    /**
     * Execute the console command.
     */
    public function handle()
    {
        $this->info('Faerie observing...');

        // See if there are any open PRs
            // If so, see if they are ready to merge (all checks passed - and also analyze the comments)
                // If so, merge them and comment on the PR
                // If not because tests are failing, add a commit to the PR that fixes the tests, and comment on the PR
                // If not because some requested feature is missing, add a commit to the PR that adds the feature, and comment on the PR
            // If no open PRs, look for any open issues
                // Analyze the conversation to see if what's needed next is a comment or a PR with code - and take that action
    }
}
