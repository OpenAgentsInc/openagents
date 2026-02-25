<?php

declare(strict_types=1);

namespace JonPurvis\Profanify\Support;

final class Russian
{
    private static string $pattern = '/[А-Яа-яЁё]+/u';

    private bool $detected = false;

    /** @var array<string, string> */
    private static array $normalized = [];

    /** @var array<int|string, string> */
    private static array $toNormalize = [
        '3' => 'з', '4' => 'ч', '6' => 'б',
        'a' => 'а', 'c' => 'с', 'e' => 'е', 'o' => 'о', 'p' => 'р', 'x' => 'х', 'k' => 'к',
        'A' => 'д', 'r' => 'г', 'H' => 'н', 'M' => 'м', 'T' => 'т', 'B' => 'в',
    ];

    public function is(string $text): bool
    {
        if ((bool) preg_match(self::$pattern, $text)) {
            $this->detected = true;
        }

        return $this->detected;
    }

    public function isDetected(): bool
    {
        return $this->detected;
    }

    public static function pattern(): string
    {
        return self::$pattern;
    }

    public static function normalize(string $text): string
    {
        preg_match_all('/\w+/u', $text, $words);
        $toNormalizeKeysString = implode('', array_keys(self::$toNormalize));

        foreach ($words[0] as $word) {
            if (strpbrk($word, $toNormalizeKeysString)) {
                $normalized = strtr($word, self::$toNormalize);
                self::$normalized[$word] = $normalized;
            }
        }

        return str_replace(array_keys(self::$normalized), array_values(self::$normalized), $text);
    }

    /**
     * @param  array<string>  $profanities
     * @return array<string>
     */
    public static function backToOrigin(array $profanities): array
    {
        return array_map(fn ($profanity): string => array_search($profanity, self::$normalized) ?: $profanity, $profanities);
    }
}
