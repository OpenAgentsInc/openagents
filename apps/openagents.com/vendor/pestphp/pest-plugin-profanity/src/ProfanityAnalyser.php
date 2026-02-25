<?php

declare(strict_types=1);

namespace Pest\Profanity;

/**
 * @internal
 */
final class ProfanityAnalyser
{
    /**
     * Scan a file for profanity
     *
     * @param  array<string>  $excludingWords
     * @param  array<string>  $includingWords
     * @param  array<string>|null  $languages
     * @return array<int, Error>
     */
    public static function analyse(string $file, array $excludingWords = [], array $includingWords = [], $languages = null): array
    {
        $words = [];
        $profanitiesDir = __DIR__.'/Config/profanities';
        $errors = [];

        if (str_contains($file, '/Config/profanities/')) {
            return [];
        }

        if (($profanitiesFiles = scandir($profanitiesDir)) === false) {
            return [];
        }

        $profanitiesFiles = array_diff($profanitiesFiles, ['.', '..']);

        if ($languages) {
            foreach ($languages as $lang) {
                $specificLanguage = "$profanitiesDir/$lang.php";
                if (file_exists($specificLanguage)) {
                    $words = array_merge(
                        $words,
                        include $specificLanguage
                    );
                }
            }
        } else {
            $words = include "$profanitiesDir/en.php";
        }

        $words = array_merge($words, $includingWords);
        $words = array_diff($words, $excludingWords);

        $fileContents = (string) file_get_contents($file);
        $lines = explode("\n", $fileContents);

        $foundProfanity = [];

        foreach ($words as $word) {
            foreach ($lines as $lineNumber => $line) {
                $key = $lineNumber.'-'.$word;
                if (preg_match('/(?<!\p{L})'.preg_quote($word, '/').'(?!\p{L})/iu', $line) === 1 && ! isset($foundProfanity[$key])) {
                    // Skip reporting profanity if the line contains the ignore annotation
                    if (! str_contains($line, '@pest-ignore-profanity')) {
                        $errors[] = new Error($file, $lineNumber + 1, $word);
                        $foundProfanity[$key] = true;
                    }
                }
            }
        }

        return $errors;
    }
}
