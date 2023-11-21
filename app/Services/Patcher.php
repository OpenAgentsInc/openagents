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
    public function getIssuePatches($issue)
    {
        $patches = [];
        $nearestFiles = $this->getNearestFiles($issue);
        dd($nearestFiles);

        foreach ($nearestFiles as $file) {
            echo "Processing file: {$file}\n";

            // Here you would have some logic to determine the 'Before' and 'After' contents
            $beforeContent = "/* predefined 'Before' content */";
            $afterContent = "/* predefined 'After' content */";

            // Read the file content
            if (!file_exists($file)) {
                echo "File not found: {$file}\n";
                continue;
            }
            $fileContent = file_get_contents($file);

            // Clean the 'Before' and 'After' contents
            $beforeContent = $this->cleanCodeBlock($beforeContent);
            $afterContent = $this->cleanCodeBlock($afterContent);

            // Check if the 'Before' content exists in the file
            if (strpos($fileContent, $beforeContent) === false) {
                echo "The specified 'Before' content was not found in the file: {$file}\n";
                continue;
            }

            // Create the patch
            $newFileContent = str_replace($beforeContent, $afterContent, $fileContent);
            $patch = [
                "file_name" => $file,
                "content" => $fileContent,
                "new_content" => $newFileContent
            ];
            $patches[] = $patch;
        }

        echo "Generated " . count($patches) . " patches for the issue.\n";
        return $patches;
    }

    /**
     * Placeholder for the getNearestFiles method.
     * It's assumed to return an array of file paths relevant to the given issue.
     *
     * @param array $issue An associative array representing an issue.
     * @return array An array of file paths.
     */
    private function getNearestFiles($issue)
    {
        // Placeholder logic: This method should contain the logic to determine the nearest files
        // For now, it returns an empty array.

        $files = $this->searcher->queryAllFiles($issue['title']);

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
}
