<?php

declare(strict_types=1);

namespace Pest\Installers;

use Pest\Support\View;

final readonly class PluginBrowser
{
    public static function install(): void
    {
        View::render('installers/plugin-browser');
    }
}
