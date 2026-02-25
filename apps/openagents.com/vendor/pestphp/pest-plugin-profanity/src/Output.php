<?php

declare(strict_types=1);

namespace Pest\Profanity;

use function Termwind\render;
use function Termwind\terminal;

/**
 * @internal
 */
class Output
{
    public static function successMessage(string $message): void
    {
        render(<<<HTML
            <div class="my-1">
                <span class="ml-2 px-1 bg-green font-bold">PASS</span>
                <span class="ml-1">$message</span>
            </div>
        HTML);
    }

    public static function errorMessage(string $message): void
    {
        render(<<<HTML
            <div class="my-1">
                <span class="ml-2 px-1 bg-red font-bold">ERROR</span>
                <span class="ml-1">$message</span>
            </div>
        HTML);
    }

    public static function pass(string $path): void
    {
        $truncateAt = max(1, terminal()->width() - 24);

        render(<<<HTML
            <div class="flex mx-2">
                <span class="truncate-{$truncateAt}">$path</span>
                <span class="flex-1 content-repeat-[.] text-gray mx-1"></span>
                <span class="text-green">OK</span>
            </div>
        HTML);
    }

    public static function fail(string $path, string $feedback): void
    {
        $truncateAt = max(1, terminal()->width() - 24);

        render(<<<HTML
            <div class="flex mx-2">
                <span class="truncate-{$truncateAt}">{$path}</span>
                <span class="flex-1 content-repeat-[.] text-gray mx-1"></span>
                <span class="text-red">$feedback</span>
            </div>
        HTML);
    }
}
