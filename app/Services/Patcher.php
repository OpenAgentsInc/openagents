<?php

namespace App\Services;

use App\Services\Searcher;
use GitHub;

class Patcher
{
    public function __construct()
    {
        $this->searcher = new Searcher();
        $this->issue = null;
        $this->task = "";
        $this->patches = [];
    }

    private function generatePrTitle()
    {
        // ask LLM to write a basic PR title based on the array of patches, $this->patches
        $prompt = "Write a PR title for the patches below. Use less than 50 characters.\n\n";
        foreach ($this->patches as $patch) {
            if ($patch === null) {
                continue;
            }
            // $prompt .= $patch['file_name'] . "\n";
            // Append file_name, content (old), and new_content (new) to the prompt
            $prompt .= $patch['file_name'] . "\n\nOld content:\n" . $patch['content'] . "\n\nNew content:\n\n" . $patch['new_content'] . "\n";
        }
        $prompt .= "\nPR title:";
        $title = $this->complete($prompt);

        // Strip off any double quotation marks at beginning and end of the title
        $title = trim($title, '"');

        // Remove all double-quotations from this string
        $title = str_replace('"', '', $title);

        return $title;
    }

    private function generatePrBody()
    {
        print_r("TASK:");
        print_r($this->task);
        print_r("-----");

        // ask LLM o write a basic PR body in Markdown based on the array of patches, $this->patches
        $prompt = "You are a senior developer about to submit a PR. You were directed to do the following task:\n\n ---\n" . $this->task . "\n---\n\n

        Write a PR body in Markdown for the patches below. Include a summary of the changes at the top, followed by a description of individual changes. Do not use the word 'patch'. Only describe the differences between the new and old content, do not summarize existing code.\n\n";

        print_r("PR BODY:");
        print_r($prompt);

        // explode by "For additional context, consult the following code snippets:"


        foreach ($this->patches as $patch) {
            if ($patch === null) {
                continue;
            }
            $prompt .= $patch['file_name'] . "\n\nOld content:\n" . $patch['content'] . "\nUpdated content:\n\n" . $patch['new_content'] . "\n";
        }
        $prompt .= "\nPR body:";
        $body = $this->complete($prompt);

        $body .= "\n\n For additional context, here were my instructions:\n\n ---\n" . $this->task;

        return $body;
    }

    private function generateCommitMessage($patch)
    {
        // ask LLM to write a basic commit message comparing the old and new content of the patch
        $prompt = "Write a commit message for the patch below. Use less than 50 characters.\n\n

        Old content:\n\n" . $patch['content'] . "\n\nNew content:\n\n" . $patch['new_content'] . "\n\nCommit message:";
        $msg = $this->complete($prompt);

        // Strip off any double quotation marks at beginning and end of the title
        $msg = trim($msg, '"');

        // Remove all double-quotations from this string
        $msg = str_replace('"', '', $msg);
        return $msg;
    }

    /**
     * Submits the given patches to GitHub as a pull request.
     *
     * @param array $patches An array of patches.
     * @param string $repository The target repository in the format 'owner/repo'.
     * @param string $branch The branch to apply the patches to.
     * @return void
     */
    public function submitPatchesToGitHub(array $patches, string $fullrepo = "ArcadeLabsInc/openagents", string $branch = 'main')
    {
        $this->patches = $patches;
        foreach ($patches as $patch) {
            if ($patch === null) {
                continue;
            }

            $path = $patch['file_name'];
            $newContent = $patch['new_content'];

            // Split fullrepo by / into org and repo
            $repo = explode("/", $fullrepo);
            $owner = $repo[0];
            $repository = $repo[1];

            try {
                // Get the reference of the branch
                $reference = GitHub::api('git')->references()->show($owner, $repository, 'heads/' . $branch);
                $sha = $reference['object']['sha'];

                $fileExists = GitHub::api('repo')->contents()->exists($owner, $repository, $path, $branch);
                $commitMessage = $this->generateCommitMessage($patch);

                if ($fileExists) {
                    // Existing file update logic
                    print_r("Updating file {$path}");
                    $fileInfo = GitHub::api('repo')->contents()->show($owner, $repository, $path, $branch);
                    $blobSha = $fileInfo['sha'];
                    GitHub::api('repo')->contents()->update($owner, $repository, $path, $newContent, $commitMessage, $blobSha, $branch);
                } else {
                    // New file creation logic
                    print_r("Creating file {$path}");
                    GitHub::api('repo')->contents()->create($owner, $repository, $path, $newContent, $commitMessage, $branch);
                }
            } catch (\Exception $e) {
                echo "Error updating file {$path}: " . $e->getMessage() . "\n";
            }
        }

        // Create pull request
        $prTitle = $this->generatePrTitle(); // Define your PR title
        $prBody = $this->generatePrBody(); // Define your PR body
        $res = GitHub::api('pull_request')->create($owner, $repository, [
            'title' => $prTitle,
            'body' => $prBody,
            'head' => $branch,
            'base' => 'main' // Base branch in the upstream repository
        ]);
        return [
            "ok" => true,
            "res" => $res
        ];
    }

    /**
     * Generates patches for a single issue based on the 'Before' and 'After' content.
     *
     * @param array $issue An associative array representing an issue,
     *                     which should include 'title', 'body', and other relevant data.
     * @return array An array of patches.
     */
    public function getIssuePatches($issue, $take = 8)
    {
        $patches = [];
        $this->issue = $issue;
        $this->task = explode("For additional context, consult the following code snippets:", $issue["body"])[0];

        // CREATE NEW FILES
        $newFiles = $this->promptForNewFiles();
        foreach ($newFiles as $newFilePath) {
            $newFileContent = $this->promptForNewFileContent($newFilePath, $issue);
            print_r("NEW FILE CONTENT!");
            print_r($newFileContent);
            $patches[] = [
                "file_name" => $newFilePath,
                "content" => '',
                "new_content" => $newFileContent
            ];
        }

        // UPDATE EXISTING FILES
        $nearestFiles = $this->getNearestFiles($issue, $take);

        foreach ($nearestFiles as $file) {
            // Determine patch for file
            $patch = $this->determinePatchForFile($file, $issue);
            $patches[] = $patch;
        }

        return $patches;
    }

    private function promptForNewFileContent($newFilePath, $issue)
    {
        $prompt = "Below is an issue on OpenAgents codebase.\nIssue: {$this->issue['title']} \n\n Issue body: {$this->issue['body']}\n";
        $actionPrompt1 = "Please enter the content for the new file {$newFilePath}. Return only code, no comments or other explanation. Place your code in Markdown backticks. Use no English words to explain, only the code in a code block.";

        $newFileContent = $this->complete($prompt . $actionPrompt1);

        return $this->cleanCodeBlock($newFileContent);
    }

    private function promptForNewFiles()
    {
        $prompt = "Below is an issue on OpenAgents codebase.\nIssue: {$this->issue['title']} \n\n Issue body: {$this->issue['body']}\n";
        $actionPrompt1 = "Please enter the file paths of any files that need to be created to resolve the issue. Separate multiple file paths with a comma.";
        $newFiles = $this->complete($prompt . $actionPrompt1);

        if ($newFiles === '') {
            return [];
        }

        // Explode the string into an array and trim each element
        $filePaths = array_map('trim', explode(",", $newFiles));

        // Filter out invalid paths and check if the file exists
        $filePaths = array_filter($filePaths, function ($path) {
            // Regular expression to validate file paths (modify as needed)
            if (!preg_match('/^[a-zA-Z0-9\/\._-]+$/', $path)) {
                return false;
            }

            // Check if the file exists
            return !file_exists($path);
        });

        return array_values($filePaths); // Re-index the array
    }

    private function determinePatchForFile($file, $issue)
    {
        // Check if the file exists
        if (!file_exists($file)) {
            dd("File not found: {$file}\n");
        }

        // Read the file content
        $fileContent = file_get_contents($file);

        // Construct the prompt for checking if a patch is needed
        $prompt = "Below is an issue on OpenAgents codebase.\nIssue: {$issue['title']} - {$issue['body']}\n\nHere is a potential file that may need to be updated to fix the issue:\n";
        $prompt .= "{$file}```\n{$fileContent}```\n";
        $actionPrompt1 = "Does this file need to be changed to resolve the issue? Respond with only `Yes` or `No`.";
        $needsPatch = $this->complete($prompt . $actionPrompt1);

        // Assuming needsPatch is 'Yes' or 'No', proceed accordingly
        if ($needsPatch === 'No') {
            return null;
        }

        // Configure a maximum number of retries
        $maxRetries = 5;
        $retryCount = 0;

        while (true) {
            print_r("Trying code block for {$file}, attempt #{$retryCount}\n");
            // Construct the prompt for getting the 'Before' and 'After' contents
            $actionPrompt2 = "Identify which code block needs to be changed (mark it up with \"Before:\") and output the change (mark it up with \"After:\"). Make your change match the coding style of the original file.";
            // print_r($prompt . $actionPrompt2 . "\n");
            $change = $this->complete($prompt . $actionPrompt2);
            // print_r($change);

            if (strpos($change, "Before:") === false || strpos($change, "After:") === false) {
                dd("Warning: incorrect output format\n");
                return null;
            }

            list($before, $after) = explode("After:", explode("Before:", $change, 2)[1], 2);
            $before = $this->cleanCodeBlock($before);
            $after = $this->cleanCodeBlock($after);

            // Use preg_quote to escape any special characters in the $before string
            $escapedBefore = preg_quote($before, '/');

            // Create a regular expression that allows for flexible matching
            $pattern = "/\s*" . str_replace("\n", "\s*", $escapedBefore) . "\s*/s";

            if (!preg_match($pattern, $fileContent)) {
                print_r("didnt match");
                if (++$retryCount >= $maxRetries) {
                    dd("Warning: exceeded maximum retries for finding `Before` block\n");
                    return null;
                }
                continue;
            }

            // Ensure that the 'after' block ends with a newline character
            if (!str_ends_with($after, "\n")) {
                $after .= "\n";
            }

            // Ensure that the 'after block begins with a newline character
            if (!str_starts_with($after, "\n")) {
                $after = "\n" . $after;
            }

            // Find and replace using preg_replace
            $newFileContent = preg_replace($pattern, $after, $fileContent, 1);

            // Additional step to ensure proper spacing between lines if necessary
            // This step can be adjusted based on the specific formatting issues you are encountering
            $newFileContent = preg_replace('/([;}])it/', "$1\nit", $newFileContent);

            return [
                "file_name" => $file,
                "content" => $fileContent,
                "new_content" => $newFileContent
            ];
        }
    }

    /**
     * Placeholder for the getNearestFiles method.
     * It's assumed to return an array of file paths relevant to the given issue.
     *
     * @param array $issue An associative array representing an issue.
     * @return array An array of file paths.
     */
    private function getNearestFiles($issue, $take = 5)
    {
        // Placeholder logic: This method should contain the logic to determine the nearest files
        // For now, it returns an empty array.

        $files = $this->searcher->queryAllFiles($issue['title'] . "\n" . $issue['body'], $take);

        // if $files["ok"] == true, then $files["results"] contains the files,each having "path" with path. Just return an array of that
        if ($files["ok"]) {
            $paths = [];
            foreach ($files["results"] as $file) {
                $paths[] = $file["path"];
            }
            return $paths;
        }

        return [];
    }

    /**
     * Cleans a code block by stripping whitespace and removing markdown code block syntax.
     *
     * @param string $codeBlock The code block to clean.
     * @return string The cleaned code block.
     */
    public function cleanCodeBlock($codeBlock)
    {
        // Trim whitespace from both ends of the string
        $codeBlock = trim($codeBlock);

        // Remove markdown code block syntax if present
        if (substr($codeBlock, 0, 6) === "```php") {
            $codeBlock = substr($codeBlock, 6);
        }
        if (substr($codeBlock, 0, 3) === "```") {
            $codeBlock = substr($codeBlock, 3);
        }
        if (substr($codeBlock, -3) === "```") {
            $codeBlock = substr($codeBlock, 0, -3);
        }

        // Trim again to remove any whitespace left after removing the syntax
        return trim($codeBlock);
    }

    /**
     * Generates a response from OpenAI's Completion API based on the provided prompt.
     *
     * @param string $prompt The prompt to send to the API.
     * @param int $tokensResponse The maximum number of tokens in the response.
     * @return string The response text from the API.
     */
    private function complete($prompt, $tokensResponse = 1024)
    {
        $maxContentLength = 4097; // Define this constant based on your use case
        $modelCompletion = "gpt-3.5-turbo-instruct"; // Define this constant for the model you're using
        // $modelCompletion = "text-davinci-003"; // Define this constant for the model you're using

        // if (strlen($prompt) > $maxContentLength - $tokensResponse) {
        //     $nonSequitur = '\n...truncated\n';
        //     $margin = intdiv(strlen($nonSequitur), 2);
        //     $firstHalf = intdiv($maxContentLength - $tokensResponse, 2);
        //     $prompt = substr($prompt, 0, $firstHalf - $margin) . $nonSequitur . substr($prompt, -$firstHalf + $margin);
        // }

        for ($i = 0; $i < 3; $i++) {
            $ch = curl_init();
            curl_setopt($ch, CURLOPT_URL, 'https://api.openai.com/v1/engines/' . $modelCompletion . '/completions');
            curl_setopt($ch, CURLOPT_RETURNTRANSFER, 1);
            curl_setopt($ch, CURLOPT_POST, 1);
            curl_setopt($ch, CURLOPT_POSTFIELDS, json_encode([
                'prompt' => $prompt,
                'max_tokens' => $tokensResponse,
                'temperature' => 0.2,
                'top_p' => 1,
                'frequency_penalty' => 0.5,
                'presence_penalty' => 0.6
            ]));
            curl_setopt($ch, CURLOPT_HTTPHEADER, [
                'Content-Type: application/json',
                'Authorization: Bearer ' . env("OPENAI_API_KEY")
            ]);

            $result = curl_exec($ch);
            if (curl_errno($ch)) {
                echo "Tried $i times. Couldn't get response, trying again...\n";
                sleep(1);
                continue;
            }

            $response = json_decode($result, true);
            curl_close($ch);

            if (isset($response['choices'][0]['text'])) {
                return trim($response['choices'][0]['text']);
            }
        }

        return '---'; // Return empty string or handle error appropriately
    }
}
