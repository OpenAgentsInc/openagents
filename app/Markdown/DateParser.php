<?php

namespace App\Markdown;

use League\CommonMark\Parser\Inline\InlineParserInterface;
use League\CommonMark\Parser\Inline\InlineParserMatch;
use League\CommonMark\Parser\InlineParserContext;

class DateParser implements InlineParserInterface
{
    public function getMatchDefinition(): InlineParserMatch
    {
        // Define the starting delimiter
        return InlineParserMatch::string('--date:');
    }

    public function parse(InlineParserContext $inlineContext): bool
    {
        $cursor = $inlineContext->getCursor();

        // dd($cursor->getLine());
        // $cursor->getLine() is "--date: '2023-04-06T00:00Z'--"

        // Extract the date string
        $dateString = substr($cursor->getLine(), 8, 20);

        // strip all dashes and single quotes
        $dateString = str_replace(['-', '\''], '', $dateString);

        // Create and add the DateElement
        $inlineContext->getContainer()->appendChild(new DateElement($dateString));

        // Advance the cursor past the trailing '--'
        // if ($cursor->peek(2) === '--') {
        $cursor->advanceBy(29);
        // }

        return true;

        // // Advance the cursor past the initial delimiter '--date:'
        // $cursor->advanceBy(8); // Length of '--date:'

        // // Skip whitespace after '--date:'
        // while ($cursor->peek() === ' ') {
        //     $cursor->advance();
        // }

        // // Assume the date string is enclosed in single quotes
        // if ($cursor->peek() === '\'') {
        //     $cursor->advance(); // Skip the opening quote

        //     $start = $cursor->getPosition();
        //     while (!$cursor->isAtEnd() && $cursor->peek() !== '\'') {
        //         $cursor->advance();
        //     }
        //     $end = $cursor->getPosition();

        //     // Extract the date string
        //     $dateString = substr($cursor->getLine(), $start, $end - $start);

        //     // Create and add the DateElement
        //     $inlineContext->getContainer()->appendChild(new DateElement($dateString));

        //     // Advance the cursor past the closing quote
        //     $cursor->advance();

        //     // Advance the cursor past the trailing '--'
        //     if ($cursor->peek(2) === '--') {
        //         $cursor->advanceBy(2);
        //     }

        //     return true;
        // }

        // return false;
    }
}
