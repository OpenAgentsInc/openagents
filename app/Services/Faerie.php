<?php

namespace App\Services;

// use App\Events\StartFaerieRun;
use App\Jobs\StartFaerieRun;
use App\Models\Agent;
use App\Models\Run;
use App\Models\Step;
use App\Models\Task;
use App\Models\User;
use GitHub;
use Illuminate\Support\Facades\Log;

class Faerie
{
    public $owner;
    public $repo;

    private $agent;
    private $task;

    private $issue;
    private $pr;

    private $gateway;

    public function __construct($owner = "OpenAgentsInc", $repo = "openagents")
    {
        $this->gateway = new OpenAIGateway();
        $this->owner = $owner;
        $this->repo = $repo;
        $user_id = auth()->user()->id ?? User::factory()->create()->id;
        $this->agent = Agent::create([
            'user_id' => $user_id,
            'name' => $this->owner . '/' . $this->repo,
        ]);
        $this->task = Task::create([
            'agent_id' => $this->agent->id,
            'description' => 'Analyze a PR and make a commit',
        ]);
        // $this->run = Run::create([
        //     'agent_id' => $this->agent->id,
        //     'task_id' => $this->task->id,
        //     'description' => 'GitHubAgent analyzes a PR and makes a commit',
        //     'status' => 'pending',
        //     'amount' => 0
        // ]);
    }

    public function log($wat)
    {
        dump($wat);
    }

    public function runJob()
    {
        // run the StartFaerieRun event
        // $this->log("Running StartFaerieRun event");
        // event(new StartFaerieRun($this));
        StartFaerieRun::dispatch($this);
        return ['status' => 'Job dispatched'];
    }

    public function run()
    {
        if(!$this->repoHasOpenPR()) {
            return ['status' => 'no_open_pr'];
        }

        $this->log("Hello from Faerie!");


        $this->fetchMostRecentPR();
        $this->log("Running Faerie on PR " . $this->pr['number']);

        $analysis = $this->analyzePr()["comment"];
        $this->log($analysis);

        if ($analysis == "TESTFIX") {
            $this->fixTests();
        } else {
            dd("Unhandled analysis: " . $analysis);
        }

        return ['status' => 'success'];
    }

    public function fixTests()
    {
        // print_r("Fixing tests...\n");
        $pr = $this->pr;

        $system = "You are Faerie, an AI agent specialized in writing & analyzing code.\n\n Please review this PR and determine why the tests failed:\n\n";
        $system .= $this->getPrSummary();
        $commits = $this->getAllCommitsFromPr($pr['number']);

        foreach ($commits as $commit) {
            $sha = $commit['sha'];
            $commit_resp = GitHub::repo()->commits()->show($this->owner, $this->repo, $sha);
            $system .= "\n\nHere is the diff for " . $commit_resp["files"][0]["filename"] . ":\n\n";
            $system .= $commit_resp["files"][0]["patch"];
        }

        $prompt = "Why did the tests fail?";

        $systemMessage = ['role' => 'system', 'content' => $system];
        $fixdescription = $this->chatComplete([
            $systemMessage,
            ['role' => 'user', 'content' => $prompt]
        ]);
        $this->recordStep("Generated fix description", $fixdescription, null);

        $patcher = new Patcher();
        $planner = new Planner();

        // Build the context from a summary of the messages passed as query to a similarity search
        $context = $this->buildContextFrom($fixdescription);
        $this->recordStep("Built context", $fixdescription, $context);

        // Create a plan from the messages
        $taskDescription = $planner->createPlan([$systemMessage]);
        $this->recordStep("Created plan", [$systemMessage], $taskDescription);

        $planPrompt = "A description of your next task is:\n" . $taskDescription . "

        For additional context, consult the following code snippets:
        ---
        " . $context;

        $patchInput = [
            "title" => $this->pr["title"],
            "body" => $planPrompt
        ];
        $patches = $patcher->getPrPatches($patchInput, $commits);
        $this->recordStep("Created patches", $patchInput, $patches);

        // // print_r("PATCHES:");
        // // print_r($patches);
        // print_r("--- . . skipping submitting");

        // $res = $patcher->submitPatchesToGitHub($patches, "OpenAgentsInc/openagents", "vid32test17", false);

        // print_r("Done!");
    }

    public function recordStep($description, $input, $output)
    {
        // $this->log("Skipped recording step: " . $description);
        // return [
        //     'status' => 'skipped',
        // ];

        // $this->log("Attempting to record step.");
        try {
            $step = Step::create([
                'agent_id' => $this->agent->id,
                'run_id' => $this->run->id,
                'description' => $description,
                'input' => json_encode($input),
                'output' => json_encode($output),
            ]);
        } catch (\Exception $e) {
            $this->log("Failed to record step: " . $e->getMessage());
            return [
                'status' => 'error',
                'message' => $e->getMessage(),
            ];
        }

        return [
            'status' => 'success',
            'step' => $step,
        ];
    }

    public function getAllCommitsFromPr($pr_number)
    {
        $url = "https://api.github.com/repos/{$this->owner}/{$this->repo}/pulls/{$pr_number}/commits";
        $response = $this->curl($url);
        $this->recordStep('Get all commits from PR', [], $response);
        return $response["response"];
    }

    public function repoHasOpenPR()
    {
        $url = "https://api.github.com/repos/{$this->owner}/{$this->repo}/pulls?state=open";
        $response = $this->curl($url);
        $this->recordStep('Check if repo has open PR', [], $response);
        return count($response) > 0;
    }

    public function fetchMostRecentPR()
    {
        $url = "https://api.github.com/repos/{$this->owner}/{$this->repo}/pulls?state=open";
        $response = $this->curl($url);
        $this->recordStep('Fetch most recent PR', [], $response);

        // And fetch the comments on that PR
        $url = "https://api.github.com/repos/{$this->owner}/{$this->repo}/issues/{$response["response"][0]['number']}/comments";
        $comments_response = $this->curl($url)["response"];
        $pr_first = $response["response"][0];

        $comments = [];
        foreach ($comments_response as $comment) {
            $comments[] = [
                "author" => $comment["user"]["login"],
                "body" => $comment["body"]
            ];
        }

        $this->pr = [
            "title" => $pr_first["title"],
            "body" => $pr_first["body"],
            "state" => $pr_first["state"],
            "number" => $pr_first["number"],
            "comments" => $comments,
        ];

        return $this->pr;
    }

    public function fetchMostRecentIssue()
    {
        $url = "https://api.github.com/repos/{$this->owner}/{$this->repo}/issues?state=open";
        $response = $this->curl($url);
        $issue = $response["response"][0];
        $this->recordStep('Fetch most recent issue', [], $issue);
        $this->issue = $issue;
        return $this->issue;
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
            // Truncate the comment body to 1000 characters. Todo: use LLM to summarize
            $commentbody = substr($comment["body"], 0, 1000);
            $summary .= "\n\n" . $comment["author"] . " said: " . $commentbody . "\n";
        }
        return $summary;
    }

    /**
     * Analyze a PR to see if it's ready to merge
     *
     * @param array $pr
     * @return bool
     */
    public function analyzePr()
    {
        $system = "You are Faerie, an AI agent specialized in writing & analyzing code.\n\n Please review this PR:";
        $system .= $this->getPrSummary();
        $prompt = "Based on the above, respond only with TESTFIX, READY_FOR_REVIEW, or COMMENT.

        TESTFIX means that the PR needs tests fixed.

        READY_FOR_REVIEW means that the PR is ready for review.

        COMMENT means that the PR needs more work and you should write a comment with additional details.";

        $messages = [
            ['role' => 'system', 'content' => $system],
            ['role' => 'user', 'content' => $prompt],
        ];

        dump($messages);

        $comment = $this->chatComplete($messages);
        dump($comment);
        return [
            'status' => 'success',
            'comment' => $comment,
        ];
    }

    private function curl($url)
    {
        $startTime = microtime(true); // Start time

        $curl = curl_init();
        curl_setopt_array($curl, [
            CURLOPT_RETURNTRANSFER => 1,
            CURLOPT_URL => $url,
            CURLOPT_USERAGENT => 'OpenAgentsInc',
            CURLOPT_HTTPHEADER => [
                'Accept: application/vnd.github.v3+json',
                'Authorization: token ' . env('GITHUB_TOKEN'),
            ],
        ]);
        $response = curl_exec($curl);
        curl_close($curl);

        $endTime = microtime(true); // End time
        $duration = ($endTime - $startTime) * 1000; // Duration in milliseconds

        // You can return the duration along with the response
        // Or you can log it, print it, or handle it as needed
        // Log::info("Request duration: " . $duration . " ms\n");

        return [
            "response" => json_decode($response, true),
            "tokens_used" => round($duration)
        ];
    }

    public function chatComplete($messages, $model = 'gpt-4')
    {
        $maxChars = 6000; // Maximum character limit
        $totalChars = 0;

        // Filter messages to stay within the character limit. @todo: make this less horribly hacky
        foreach ($messages as $index => $message) {
            $messageLength = strlen($message['content']);
            $totalChars += $messageLength;
            $minCharsPerMessage = 2000; // Minimum characters per message

            if ($totalChars > $maxChars) {
                $messages[$index]['content'] = substr($message['content'], 0, $minCharsPerMessage);
                break;
            }
        }

        // Loop through each message to ensure it is valid UTF-8 and filter out invalid messages
        foreach ($messages as $index => $message) {
            $message['content'] = mb_convert_encoding($message['content'], 'UTF-8', 'UTF-8');
            if (mb_check_encoding($message['content'], 'UTF-8')) {
                $messages[$index]['content'] = $message['content'];
            } else {
                // dd("INVALID MESSAGE");
                unset($messages[$index]);
            }
        }

        // Filter messages to stay within the character limit. @todo: make this less horribly hacky
        // foreach ($messages as $index => $message) {
        //     $messageLength = strlen($message['content']);
        //     $totalChars += $messageLength;
        //     $minCharsPerMessage = 2000; // Minimum characters per message

        //     if ($totalChars > $maxChars) {
        //         $messages[$index]['content'] = substr($message['content'], 0, $minCharsPerMessage);
        //         break;
        //     }
        // }
        // echo "Total chars: $totalChars\n";

        // dd($messages);

        $input = [
            'model' => $model,
            'messages' => $messages,
            // "messages" => [
            //     [
            //         "role" => "system",
            //         "content" => "Hello, I'm a chatbot that can help you find files. What would you like to search for?"
            //     ],
            //     [
            //         "role" => "user",
            //         "content" => "I'm looking for a file about the new product launch."
            //     ]
            // ],
        ];

        // // print_r($input);
        // dump("Attempting to make chat completion");
        $response = $this->gateway->makeChatCompletion($input);
        // dump($response);

        // // print_r($response);
        try {
            $output = $response['choices'][0];
            $comment = $output['message']['content'];
            $this->recordStep('LLM chat completion', $input, [
                "response" => $output,
                "usage" => $response["usage"]
            ]);
        } catch (\Exception $e) {
            $comment = $e->getMessage();
            $this->recordStep('LLM chat completion error', $input, [
                "response" => $comment,
                "usage" => $response["usage"]
            ]);
        }

        return $comment;
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
        $query = $this->chatComplete([
            ['role' => 'system', 'content' => 'You are a helpful assistant. Speak concisely. Answer the user\'s question based only on the following context: ' . $queryInput],
            ['role' => 'user', 'content' => 'Write 2-3 sentences explaining the types of files we should search for in our codebase to provide the next response in the conversation. Focus only on the next step, not future optimizations. Ignore mentions of video transcriptions or readme/documentation.'],
        ]);

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
}
