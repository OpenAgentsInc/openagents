<?php

namespace App\Services;

use Illuminate\Support\HtmlString;
use League\CommonMark\CommonMarkConverter;
use Spatie\Sheets\ContentParser;
use Spatie\YamlFrontMatter\YamlFrontMatter;

class MarkdownParser implements ContentParser
{
    /** @var \League\CommonMark\CommonMarkConverter */
    protected $commonMarkConverter;

    public function __construct(CommonMarkConverter $commonMarkConverter)
    {
        $this->commonMarkConverter = $commonMarkConverter;
    }

    public function parse(string $contents): array
    {
        $document = YamlFrontMatter::parse($contents);

        // Convert Markdown to HTML
        $htmlContents = $this->commonMarkConverter->convertToHtml($document->body());

        // Replace all <a href="..." with <a wire:navigate href="..."
        $updatedHtmlContents = $this->replaceAnchorTags($htmlContents);

        return array_merge(
            $document->matter(),
            ['contents' => new HtmlString($updatedHtmlContents)]
        );
    }

    protected function replaceAnchorTags(string $htmlContents): string
    {
        return preg_replace('/<a (.*?)href="/i', '<a $1wire:navigate href="', $htmlContents);
    }
}
