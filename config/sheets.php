<?php

use App\Services\MarkdownParser;

return [
    'collections' => [
        'docs' => [
            'content_parser' => MarkdownParser::class,
            //'content_parser' => Spatie\Sheets\ContentParsers\MarkdownWithFrontMatterParser::class,
        ]
    ],
];
