<?php

namespace App\Console\Commands;

use App\Services\OpenAIGateway;
use GitHub;
use Illuminate\Console\Command;

class FaerieStep extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'faerie:step {--org=} {--repo=} {--pr=}';

    /**
     * The console command description.
     *
     * @var string
     */
    protected $description = 'Makes Faerie advance a step';

    private $org;
    private $repo;
    private $pr_number;
    private $pr;

    /**
     * Execute the console command.
     */
    public function handle()
    {
        $this->org = $this->option('org') ?? 'ArcadeLabsInc';
        $this->repo = $this->option('repo') ?? 'openagents';
        $this->pr_number = $this->option('pr') ?? 14;
        $this->pr = $this->getPr();

        // Analyze the PR to see if it's ready to merge
        $analysis = $this->analyzePr();
        dd();

        // If so, see if they are ready to merge (all checks passed - and also analyze the comments)
        if ($analysis["ready"]) {
            // If so, merge them and comment on the PR
            $this->info('PR is ready to merge');
        // TODO: merge PR
        } elseif ($analysis["reason"] == "Tests failed") {
            // If not because tests are failing, add a commit to the PR that fixes the tests, and comment on the PR
            $this->info('PR is not ready to merge because tests are failing - adding a commit to fix the tests');
            $fix = $this->fixTests();
        } else {
            // If not because some requested feature is missing, add a commit to the PR that adds the feature, and comment on the PR
            $this->info('PR is not ready to merge - no handler for this reason');
        }
    }

    /**
     * Add a commit to the PR that fixes the tests
     *
     * @param array $pr
     * @return void
     */
    private function fixTests()
    {
        $this->info('Fixing tests...');
        $pr = $this->pr;

        $system = "You are Faerie, an AI agent specialized in writing & analyzing code.\n\n Please review this PR:";
        $system .= $this->getPrSummary();
        $prompt = "Summarize the status of the PR.";

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
     * Get a summary of the PR
     *
     * @param array $pr
     * @return string
     */
    private function getPrSummary()
    {
        $pr = $this->pr;
        $summary = "The PR is titled '" . $pr["title"] . "' and is " . $pr["state"] . ".\n\n";
        $summary .= "There are " . count($pr["comments"]) . " comments on this PR. They are:\n\n";
        foreach ($pr["comments"] as $comment) {
            $summary .= "\n\n" . $comment["author"] . " said: " . $comment["body"] . "\n";
        }
        return $summary;
    }

    /**
     * Fetch the PR and conversation
     *
     * @return array
     */
    private function getPr()
    {
        $this->info('Getting PR...');
        $response = GitHub::issues()->show($this->org, $this->repo, $this->pr_number);
        $comments_response = GitHub::api('issue')->comments()->all($this->org, $this->repo, $this->pr_number);
        $comments = [];
        foreach ($comments_response as $comment) {
            $comments[] = [
                "author" => $comment["user"]["login"],
                "body" => $comment["body"]
            ];
        }

        $pr = [
            "title" => $response["title"],
            "body" => $response["body"],
            "state" => $response["state"],
            "comments" => $comments
        ];

        $this->info('Done getting ' . $pr["state"] . ' PR titled "' . $pr["title"] . '" with ' . count($pr["comments"]) . ' comments');
        return $pr;
    }

    /**
     * Analyze a PR to see if it's ready to merge
     *
     * @param array $pr
     * @return bool
     */
    private function analyzePr()
    {
        $this->info('Analyzing PR...');
        $system = "You are Faerie, an AI agent specialized in writing & analyzing code.\n\n Please review this PR:";
        $system .= $this->getPrSummary();
        $prompt = "Summarize the status of the PR.";

        $gateway = new OpenAIGateway();
        $response = $gateway->makeChatCompletion([
          'model' => 'gpt-4',
          'messages' => [
            ['role' => 'system', 'content' => $system],
            ['role' => 'user', 'content' => $prompt],
          ],
        ]);
        $comment = $response['choices'][0]['message']['content'];
        print_r($comment);

        return [
            "ready" => true,
            "reason" => "All checks passed"
        ];
    }
}
