<?php

namespace App\Services;

use App\Models\Agent;
use App\Models\Step;
use App\Models\Task;
use App\Models\User;

class Faerie {
    public $owner;
    public $repo;

    private $agent;
    private $task;

    private $issue;
    private $pr;

    private $gateway;

    public function __construct($owner = "ArcadeLabsInc", $repo = "openagents") {
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
            'description' => 'Test task',
        ]);
    }

    public function run() {
        // If there is an open PR, analyze it
        if($this->repoHasOpenPR()) {
            $this->fetchMostRecentPR();
            $this->analyzePr();
        // Otherwise, analyze the most recent issue
        } else {
            $this->fetchMostRecentIssue();
            $this->analyzeIssue();
        }

        return [
            'status' => 'success',
        ];
    }

    public function recordStep($description, $input, $output) {
        $step = Step::create([
            'agent_id' => $this->agent->id,
            'task_id' => $this->task->id,
            'description' => $description,
            'input' => json_encode($input),
            'output' => json_encode($output),
        ]);
        return [
            'status' => 'success',
            'step' => $step,
        ];
    }

    public function repoHasOpenPR() {
        $url = "https://api.github.com/repos/{$this->owner}/{$this->repo}/pulls?state=open";
        $response = $this->curl($url);
        $this->recordStep('Check if repo has open PR', [], $response);
        return count($response) > 0;
    }

    public function fetchMostRecentPR() {
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

    public function fetchMostRecentIssue() {
        $url = "https://api.github.com/repos/{$this->owner}/{$this->repo}/issues?state=open";
        $response = $this->curl($url);
        $this->recordStep('Fetch most recent issue', [], $response);
        $this->issue = $response["response"][0];
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
            $summary .= "\n\n" . $comment["author"] . " said: " . $comment["body"] . "\n";
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

        $comment = $this->chatComplete($messages);
        return [
            'status' => 'success',
            'comment' => $comment,
        ];
    }

    private function curl ($url) {
        $startTime = microtime(true); // Start time

        $curl = curl_init();
        curl_setopt_array($curl, [
            CURLOPT_RETURNTRANSFER => 1,
            CURLOPT_URL => $url,
            CURLOPT_USERAGENT => 'ArcadeLabsInc',
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
        echo "Request duration: " . $duration . " ms\n";

        return [
            "response" => json_decode($response, true),
            "tokens_used" => round($duration)
        ];
    }

    public function chatComplete($messages, $model = 'gpt-4')
    {
        $maxChars = 6000; // Maximum character limit
        $totalChars = 0;

        // Filter messages to stay within the character limit
        foreach ($messages as $index => $message) {
            $messageLength = strlen($message['content']);
            $totalChars += $messageLength;
            $minCharsPerMessage = 2000; // Minimum characters per message

            if ($totalChars > $maxChars) {
                // Truncate the array up to the current index
                // $messages = array_slice($messages, 0, $index);
                // Truncate each message to the minimum character length

                $messages[$index]['content'] = substr($message['content'], 0, $minCharsPerMessage);
                break;
            }
        }
        echo "Total chars: $totalChars\n";

        $input = [
            'model' => $model,
            'messages' => $messages,
        ];

        print_r($input);
        $response = $this->gateway->makeChatCompletion($input);
        print_r($response);
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
}
