<?php

namespace App\Services;

use RecursiveDirectoryIterator;
use RecursiveIteratorIterator;

class CodeAnalyzer
{
    public static function getAllCodebaseFilePaths(string $baseDir): array
    {
        $directory = new RecursiveDirectoryIterator($baseDir, RecursiveDirectoryIterator::SKIP_DOTS);
        $iterator = new RecursiveIteratorIterator($directory, RecursiveIteratorIterator::SELF_FIRST);
        $filePaths = [];

        foreach ($iterator as $info) {
            if ($info->isFile()) {
                // Get the file's full path
                $fullPath = $info->getPathname();

                // Skip files in excluded directories
                if (strpos($fullPath, DIRECTORY_SEPARATOR.'node_modules'.DIRECTORY_SEPARATOR) !== false ||
                    strpos($fullPath, DIRECTORY_SEPARATOR.'vendor'.DIRECTORY_SEPARATOR) !== false ||
                    strpos($fullPath, DIRECTORY_SEPARATOR.'storage'.DIRECTORY_SEPARATOR) !== false) {
                    continue;
                }

                // Get the file extension
                $extension = pathinfo($info->getFilename(), PATHINFO_EXTENSION);

                // Only include files that end in .php or .md
                if (in_array($extension, ['md'])) { // 'php',
                    // Get relative path from base directory
                    $relativePath = str_replace($baseDir.DIRECTORY_SEPARATOR, '', $info->getPathname());
                    $filePaths[] = $relativePath;
                }
            }
        }

        return $filePaths;
    }

    public static function generateContext(array $filepaths): string
    {
        $prompt = '';

        foreach ($filepaths as $filepath) {
            $prompt .= "### File: {$filepath}\n\n```".
                file_get_contents($filepath)."```\n\n";
        }

        return $prompt;
    }
}
