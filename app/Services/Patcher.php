<?php

namespace App\Services;

use App\Services\Searcher;

class Patcher
{
    public function __construct()
    {
        $this->searcher = new Searcher();
    }

    /**
     * Generates patches for a single issue based on the 'Before' and 'After' content.
     *
     * @param array $issue An associative array representing an issue,
     *                     which should include 'title', 'body', and other relevant data.
     * @return array An array of patches.
     */
    public function getIssuePatches($issue, $take = 5)
    {
        $patches = [];
        $nearestFiles = $this->getNearestFiles($issue, $take);

        foreach ($nearestFiles as $file) {
            // Determine patch for file
            $patch = $this->determinePatchForFile($file, $issue);
            $patches[] = $patch;
        }

        return $patches;
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

        // Construct the prompt for getting the 'Before' and 'After' contents
        $actionPrompt2 = "Identify which code block needs to be changed (mark it up with \"Before:\") and output the change (mark it up with \"After:\"). Make your change match the coding style of the original file.";
        $change = $this->complete($prompt . $actionPrompt2);

        if (strpos($change, "Before:") === false || strpos($change, "After:") === false) {
            dd("Warning: incorrect output format\n");
            return null;
        }

        list($before, $after) = explode("After:", explode("Before:", $change, 2)[1], 2);
        $before = $this->cleanCodeBlock($before);
        $after = $this->cleanCodeBlock($after);

        if (strpos($fileContent, $before) === false) {
            dd("Warning: cannot locate `Before` block\n");
            return null;
        }

        $newFileContent = str_replace($before, $after, $fileContent);
        return [
            "file_name" => $file,
            "content" => $fileContent,
            "new_content" => $newFileContent
        ];
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

        if (strlen($prompt) > $maxContentLength - $tokensResponse) {
            $nonSequitur = '\n...truncated\n';
            $margin = intdiv(strlen($nonSequitur), 2);
            $firstHalf = intdiv($maxContentLength - $tokensResponse, 2);
            $prompt = substr($prompt, 0, $firstHalf - $margin) . $nonSequitur . substr($prompt, -$firstHalf + $margin);
        }

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
