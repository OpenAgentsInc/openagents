<?php

namespace App\Console\Commands;

use App\Services\OpenAIGateway;
use App\Services\Patcher;
use App\Services\Planner;
use App\Services\Searcher;
use GitHub;
use Illuminate\Console\Command;

class FaerieStep extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'faerie:step {--org=} {--repo=}';

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
    private $issue_number;
    private $issue;

    /**
     * Execute the console command.
     */
    public function handle()
    {
        $this->org = $this->option('org') ?? 'ArcadeLabsInc';
        $this->repo = $this->option('repo') ?? 'openagents';

        // See if there's an open PR
        $this->pr = $this->getPr();

        if ($this->pr == false) {
            $this->info('No open PRs, checking for open issues...');
            $this->issue = $this->getIssue();

            // Analyze the conversation to see if what's needed next is a comment or a PR with code - and take that action
            $this->info('Analyzing issue...');
            $this->analyzeIssue();

            return;
        }

        // Analyze the PR to see if it's ready to merge
        $analysis = $this->analyzePr();

        if ($analysis == "TESTFIX") {
            $this->info('PR needs tests fixed.');
            $this->fixTests();
            return false;
        } elseif ($analysis == "READY_FOR_REVIEW") {
            $this->info('PR is ready to merge.');
            return true;
        } elseif ($analysis == "COMMENT") {
            $this->info('PR needs more work - lets comment - unimplemented');
            return false;
        } else {
            $this->info('Unknown response');
            return false;
        }

    }

    /**
     * Analyze issue and conversation to see if what's needed next is a comment or a PR with code - and take that action
     *
     * @return void
     */
    private function analyzeIssue()
    {
        $issue = $this->issue;

        $messages = [];

        $messages[] = ['role' => 'user', 'content' => $issue["body"]];

        foreach ($issue["comments"] as $comment) {
            $role = $comment['author'] === 'FaerieAI' ? 'assistant' : 'user';
            $messages[] = ['role' => $role, 'content' => $comment['body']];
        }

        $system = "You are Faerie, an AI agent specialized in writing & analyzing code.\n\n Please review the following GitHub issue conversation and respond with only one word all in caps: CODE if you are ready to write the code, or COMMENT if you need to write a comment requesting more information. Only respond with the word CODE or COMMENT, nothing else.";

        $gateway = new OpenAIGateway();
        $response = $gateway->makeChatCompletion([
          'model' => 'gpt-4',
          'messages' => [
            ['role' => 'system', 'content' => $system],
            ...$messages,
            ['role' => 'user', 'content' => 'Based on the above, respond only with CODE or COMMENT. (CODE if you are ready to write the code, or COMMENT if you need to write a comment requesting more information. Only respond with the word CODE or COMMENT, nothing else.)'],
          ],
        ]);
        $comment = $response['choices'][0]['message']['content'];

        if ($comment == "CODE") {
            $this->info('Faerie is ready to write code');
            $this->writeCode();
        } elseif ($comment == "COMMENT") {
            $this->info('Faerie needs to write a comment');
            dd('unimplemented');
        // $this->writeComment();
        } else {
            $this->info('Faerie did not respond with CODE or COMMENT. Retrying...');
            $this->analyzeIssue();
        }

        return true;
    }

    private function getAllCommitsFromPr(int $prNumber): array
    {
        return GitHub::pullRequest()->commits($this->org, $this->repo, $prNumber);
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

        $system = "You are Faerie, an AI agent specialized in writing & analyzing code.\n\n Please review this PR and determine why the tests failed:\n\n";
        $system .= $this->getPrSummary();
        $commits = $this->getAllCommitsFromPr($pr['number']);

        // For each commit, get the SHA
        // For each commit, get the diff
        foreach ($commits as $commit) {
            $sha = $commit['sha'];
            $commit_resp = GitHub::repo()->commits()->show($this->org, $this->repo, $sha);
            $system .= "\n\nHere is the diff for " . $commit_resp["files"][0]["filename"] . ":\n\n";
            // dd($commit_resp["files"][0]["patch"]);
            $system .= $commit_resp["files"][0]["patch"];
            // $diff = GitHub::repo()->commits()->compare($this->org, $this->repo, $sha);
        }

        $prompt = "Why did the tests fail?";

        $systemMessage = ['role' => 'system', 'content' => $system];

        $gateway = new OpenAIGateway();
        $response = $gateway->makeChatCompletion([
          'model' => 'gpt-4',
          'messages' => [
            $systemMessage,
            ['role' => 'user', 'content' => $prompt],
          ],
        ]);
        $fixdescription = $response['choices'][0]['message']['content'];


        $patcher = new Patcher();
        $planner = new Planner();

        // Build the context from a summary of the messages passed as query to a similarity search
        $context = $this->buildContextFrom($fixdescription);

        // Create a plan from the messages
        $taskDescription = $planner->createPlan([$systemMessage]);

        $planPrompt = "A description of your next task is:\n" . $taskDescription . "

        For additional context, consult the following code snippets:
        ---
        " . $context;

        $patches = $patcher->getPrPatches([
            "title" => $this->pr["title"],
            "body" => $planPrompt
        ], $commits);

        print_r("PATCHES:");
        print_r($patches);
        print_r("---");

        $res = $patcher->submitPatchesToGitHub($patches, "ArcadeLabsInc/openagents", "vid32test6", false);
        print_r("RESPONSE:");
        print_r($res);

        $this->info("Done!");
    }

    /**
     * Execute the console command.
     */
    public function writeCode()
    {
        // Grab the issue body and comments from GitHub
        $response = GitHub::issues()->show('ArcadeLabsInc', 'openagents', $this->issue_number);
        $commentsResponse = GitHub::api('issue')->comments()->all('ArcadeLabsInc', 'openagents', $this->issue_number);
        $body = $response['body'];
        $title = $response['title'];

        $patcher = new Patcher();
        $planner = new Planner();

        // Format the issue and comments as messages
        $userAndAssistantMessages = $planner->formatIssueAndCommentsAsMessages($body, $commentsResponse);

        // Build the context from a summary of the messages passed as query to a similarity search
        $context = $this->buildContextFrom($userAndAssistantMessages);

        // Create a plan from the messages
        $taskDescription = $planner->createPlan($userAndAssistantMessages);

        $planPrompt = "A description of your next task is:\n" . $taskDescription . "

  For additional context, consult the following code snippets:
  ---
  " . $context;

        $patches = $patcher->getIssuePatches([
            "title" => $title,
            "body" => $planPrompt
        ]);

        print_r("PATCHES:");
        print_r($patches);
        print_r("---");

        $res = $patcher->submitPatchesToGitHub($patches, "ArcadeLabsInc/openagents", "vid32test6");
        print_r("RESPONSE:");
        print_r($res);

        $this->info("Done!");
    }

    private function buildContextFrom($input): string
    {
        $queryInput = '';

        if (is_array($input)) {
            // Original handling for arrays
            foreach ($input as $message) {
                $queryInput .= $message['content'] . "\n---\n";
            }
        } elseif (is_string($input)) {
            // If the input is a string, use it directly
            $queryInput = $input;
        }

        // Remaining part of the function stays the same
        $gateway = new OpenAIGateway();
        $response = $gateway->makeChatCompletion([
            'model' => 'gpt-4',
            'messages' => [
                ['role' => 'system', 'content' => 'You are a helpful assistant. Speak concisely. Answer the user\'s question based only on the following context: ' . $queryInput],
                ['role' => 'user', 'content' => 'Write 2-3 sentences explaining the types of files we should search for in our codebase to provide the next response in the conversation. Focus only on the next step, not future optimizations. Ignore mentions of video transcriptions or readme/documentation.'],
            ],
        ]);
        $query = $response['choices'][0]['message']['content'];

        $searcher = new Searcher();
        $results = $searcher->queryAllFiles($query);

        $context = '';
        foreach ($results["results"] as $result) {
            $context .= "Content of " . $result['path'] . ": \n```\n";
            $content = $this->getFileContent($result['path']);
            $context .= $content . "\n```\n\n";
        }

        return $context;
    }

    private function getFileContent(string $path): string
    {
        $file = fopen($path, 'r');
        $content = fread($file, filesize($path));
        fclose($file);

        return $content;
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

        // Pull all open PRs
        $prs = GitHub::pullRequests()->all($this->org, $this->repo, ['state' => 'open']);
        if (count($prs) == 0) {
            return false;
        }
        $this->pr_number = $prs[0]["number"];

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
            "number" => $response["number"], // "number" is the PR number, "id" is the issue number
            "comments" => $comments
        ];

        $this->info('Done getting ' . $pr["state"] . ' PR titled "' . $pr["title"] . '" with ' . count($pr["comments"]) . ' comments');
        return $pr;
    }

    /**
     * Fetch the issue and conversation
     * @return array
     */
    private function getIssue()
    {
        $this->info('Getting issue...');

        // Pull all open issues
        $issues = GitHub::api('issue')->all($this->org, $this->repo, array('state' => 'open'));
        if (count($issues) == 0) {
            return false;
        }
        $this->issue_number = $issues[0]["number"];

        $response = GitHub::issues()->show($this->org, $this->repo, $this->issue_number);
        $comments_response = GitHub::api('issue')->comments()->all($this->org, $this->repo, $this->issue_number);
        $comments = [];
        foreach ($comments_response as $comment) {
            $comments[] = [
                "author" => $comment["user"]["login"],
                "body" => $comment["body"]
            ];
        }

        $issue = [
            "title" => $response["title"],
            "body" => $response["body"],
            "state" => $response["state"],
            "comments" => $comments
        ];

        $this->info('Done getting ' . $issue["state"] . ' issue titled "' . $issue["title"] . '" with ' . count($issue["comments"]) . ' comments');
        return $issue;
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
        // $prompt = "Summarize the status of the PR as eit";
        $prompt = "Based on the above, respond only with TESTFIX, READY_FOR_REVIEW, or COMMENT.

        TESTFIX means that the PR needs tests fixed.

        READY_FOR_REVIEW means that the PR is ready for review.

        COMMENT means that the PR needs more work and you should write a comment with additional details.";

        $gateway = new OpenAIGateway();
        $response = $gateway->makeChatCompletion([
          'model' => 'gpt-4',
          'messages' => [
            ['role' => 'system', 'content' => $system],
            ['role' => 'user', 'content' => $prompt],
          ],
        ]);
        $comment = $response['choices'][0]['message']['content'];
        return $comment;
    }
}
