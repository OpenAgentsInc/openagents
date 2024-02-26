<?php

namespace App\Services;

use Illuminate\Support\HtmlString;
use Illuminate\Support\Facades\Blade;
use League\CommonMark\MarkdownConverter;
use League\CommonMark\CommonMarkConverter;
use League\CommonMark\Environment\Environment;
use League\CommonMark\Extension\CommonMark\CommonMarkCoreExtension;
use Spatie\CommonMarkShikiHighlighter\HighlightCodeExtension;
use Spatie\Sheets\ContentParser;
use Spatie\YamlFrontMatter\YamlFrontMatter;

class MarkdownParser implements ContentParser
{
    protected $commonMarkConverter;

    public function __construct(string $theme = 'tokyo-night')
    {
        // Create the CommonMark environment with the default configuration
        $config = [];
        $environment = new Environment($config);

        // Add the CommonMark core extension and Shiki syntax highlighting extension
        $environment->addExtension(new CommonMarkCoreExtension());
        $environment->addExtension(new HighlightCodeExtension($theme));

        // Create a Markdown converter with the configured environment
        $this->commonMarkConverter = new MarkdownConverter($environment);
    }

    public function parse(string $contents): array
    {
        $document = YamlFrontMatter::parse($contents);

        // Preprocess: Replace Livewire tags with placeholders
        $preprocessedContent = $this->preprocessContent($document->body());

        // Convert Markdown to HTML
        $htmlContents = $this->commonMarkConverter->convertToHtml($preprocessedContent);

        // Postprocess: Replace placeholders with original Livewire tags
        $htmlContentsWithComponents = $this->postprocessContent($htmlContents);

        // Further process to add wire:navigate to links
        $htmlContentsWithModifiedLinks = $this->modifyLinks($htmlContentsWithComponents);

        // Use Blade to render the content, ensuring Livewire components are processed
        $contents = Blade::render($htmlContentsWithModifiedLinks);

        return array_merge(
            $document->matter(),
            ['contents' => new HtmlString($contents)]
        );
    }

    protected function preprocessContent(string $content): string
    {
        // Example placeholder replacement for Livewire components
        $content = preg_replace('/<livewire:([^>]+)\/>/', 'LIVEWIREPLACEHOLDERSTART$1LIVEWIREPLACEHOLDEREND', $content);
        return $content;
    }

    protected function postprocessContent(string $content): string
    {
        // Replace placeholders back to Livewire tags
        $content = preg_replace('/LIVEWIREPLACEHOLDERSTART([^L]+)LIVEWIREPLACEHOLDEREND/', '<livewire:$1/>', $content);
        return $content;
    }

    protected function modifyLinks(string $content): string
    {
        // Add wire:navigate attribute to all <a href="..."> tags
        return preg_replace('/<a (.*?)href="/i', '<a $1wire:navigate href="', $content);
    }
}
