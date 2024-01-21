<?php

declare(strict_types=1);

namespace App\Markdown;

use League\CommonMark\Environment\EnvironmentBuilderInterface;
use League\CommonMark\Extension\ExtensionInterface;
use League\CommonMark\Renderer\HtmlDecorator;
use League\Config\ConfigurationBuilderInterface;

final class DateExtension implements ExtensionInterface
{
    public function register(EnvironmentBuilderInterface $environment): void
    {
        // Register your custom inline parser for dates
        $environment->addInlineParser(new DateParser());

        // Register your custom inline renderer for dates
        $environment->addRenderer(DateElement::class, new DateRenderer());
    }
}
