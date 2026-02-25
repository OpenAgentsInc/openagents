<?php

declare(strict_types=1);

namespace Pest\Profanity;

/**
 * @internal
 */
class Validator
{
    /**
     * Validates that the specified languages exist in the profanities directory.
     *
     * @param  array<string>|null  $languages
     * @return array<int, string> List of languages that don't exist
     */
    public static function validateLanguages(?array $languages): array
    {
        if ($languages === null) {
            return [];
        }

        $profanitiesDir = __DIR__.'/Config/profanities';
        $invalidLanguages = [];

        foreach ($languages as $language) {
            $specificLanguage = "$profanitiesDir/$language.php";
            if (! file_exists($specificLanguage)) {
                $invalidLanguages[] = $language;
            }
        }

        return $invalidLanguages;
    }
}
