<?php

namespace App\Services;

class Patcher
{
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
