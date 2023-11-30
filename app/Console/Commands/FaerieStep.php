<?php

namespace App\Console\Commands;

use App\Services\OpenAIGateway;
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
        $openPrs = $this->getOpenPrs();
        if (count($openPrs) > 0) {
            $this->info('Found ' . count($openPrs) . ' open PRs');
            foreach ($openPrs as $pr) {
                $this->info('PR: ' . $pr["title"]);

                // Analyze the PR to see if it's ready to merge
                $analysis = $this->analyzePr($pr);

                // If so, see if they are ready to merge (all checks passed - and also analyze the comments)
                if ($analysis["ready"]) {
                    // If so, merge them and comment on the PR
                    $this->info('PR is ready to merge');
                    // TODO: merge PR
                } else if ($analysis["reason"] == "Tests failed") {
                    // If not because tests are failing, add a commit to the PR that fixes the tests, and comment on the PR
                    $this->info('PR is not ready to merge because tests are failing - adding a commit to fix the tests');
                    $fix = $this->fixTests($pr);
                } else {
                  // If not because some requested feature is missing, add a commit to the PR that adds the feature, and comment on the PR
                  $this->info('PR is not ready to merge - no handler for this reason');
                }
            }
        } else {
            $this->info('No open PRs found');
            // If no open PRs, look for any open issues
            // Analyze the conversation to see if what's needed next is a comment or a PR with code - and take that action
        }
    }

    /**
     * Add a commit to the PR that fixes the tests
     *
     * @param array $pr
     * @return void
     */
    private function fixTests($pr)
    {
        $this->info('Fixing tests...');

        $system = "You are Faerie, an AI agent specialized in writing & analyzing code. Here is the PR title and body: \n\n" . $pr["title"] . "\n\n" . $pr["body"];
        // Append all the comments to the system message
        foreach ($pr["comments"] as $comment) {
            $system .= "\n\n" . $comment["author"] . " said: " . $comment["body"];
        }

        $prompt = "Write the code that fixes the tests and commit it to the PR.";

        print_r($system);

        // $gateway = new OpenAIGateway();
        // $response = $gateway->makeChatCompletion([
        //   'model' => 'gpt-4',
        //   'messages' => [
        //     ['role' => 'system', 'content' => $system],
        //     ['role' => 'user', 'content' => $prompt],
        //   ],
        // ]);
        // $comment = $response['choices'][0]['message']['content'];
        // print_r($comment);
        // expect($comment)->toBeString();

        // $this->info('Done fixing tests');
    }

    /**
     * Get all open PRs
     *
     * @return array
     */
    private function getOpenPrs()
    {
        $prs = [
            [
                "title" => "Test PR",
                "body" => "This is a test PR",
                "comments" => [
                    [
                        "author" => "FaerieAI",
                        "body" => "This is a test comment"
                    ]
                ]
            ]
        ];
        $this->info('Getting open PRs...');
        $this->info('Done getting open PRs');
        return $prs;
    }

    /**
     * Analyze a PR to see if it's ready to merge
     *
     * @param array $pr
     * @return bool
     */
    private function analyzePr($pr)
    {
        $this->info('Analyzing PR...');
        $this->info('Done analyzing PR');
        return [
            "ready" => false,
            "reason" => "Tests failed"
        ];
        // return [
        //     "ready" => true,
        //     "reason" => "All checks passed"
        // ];
    }
}
