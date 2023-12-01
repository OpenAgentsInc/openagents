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

        dd($comment);

        if ($comment == "CODE") {
            $this->info('Faerie is ready to write code');
            $this->writeCode();
        } elseif ($comment == "COMMENT") {
            $this->info('Faerie needs to write a comment');
            dd('unimplemented');
            // $this->writeComment();
        } else {
            $this->info('User did not respond with CODE or COMMENT');
            dd();
        }

        return true;
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
            "comments" => $comments
        ];

        $this->info('Done getting ' . $pr["state"] . ' PR titled "' . $pr["title"] . '" with ' . count($pr["comments"]) . ' comments');
        return $pr;
    }

    // get issue
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

        $res = $patcher->submitPatchesToGitHub($patches, "ArcadeLabsInc/openagents", "vid32test1");
        print_r("RESPONSE:");
        print_r($res);

        $this->info("Done!");
    }

    private function buildContextFrom(array $messages): string
    {
        $queryInput = '';

        // todo: add author to each message?
        foreach ($messages as $message) {
            $queryInput .= $message['content'] . "\n---\n";
        }

        // Let's do one LLM call to summarize queryInput with an emphasis on the types of files we need to look up
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

        // Loop through the results, and for each one, add the file name and the entire content of the file to the context
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
}
