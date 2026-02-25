<?php

declare(strict_types=1);

namespace Pest\Profanity;

use Closure;

/**
 * @internal
 */
final class Analyser
{
    /**
     * Analyse the code for profanity.
     *
     * @param  array<int, string>  $files
     * @param  \Closure(\Pest\Profanity\Result): void  $callback
     * @param  array<string>  $excludingWords
     * @param  array<string>  $includingWords
     * @param  array<string>|null  $languages
     */
    public static function analyse(
        array $files,
        Closure $callback,
        array $excludingWords = [],
        array $includingWords = [],
        $languages = null
    ): void {
        foreach ($files as $file) {
            $errors = ProfanityAnalyser::analyse($file, $excludingWords, $includingWords, $languages);
            $callback(new Result($file, $errors));
        }
    }
}
