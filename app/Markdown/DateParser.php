<?php

namespace App\Markdown;

use League\CommonMark\Parser\Inline\InlineParserInterface;
use League\CommonMark\Parser\Inline\InlineParserMatch;
use League\CommonMark\Parser\InlineParserContext;

class DateParser implements InlineParserInterface
{
    public function getMatchDefinition(): InlineParserMatch
    {
        // Define the pattern that this parser should match
        return InlineParserMatch::regex('/{{\s*date:\s*\'[0-9\-T:Z]+\'\s*}}/');
    }

    public function parse(InlineParserContext $inlineContext): bool
    {
        $cursor = $inlineContext->getCursor();

        // The cursor will already be positioned at the start of the match
        $match = $cursor->match('/{{\s*date:\s*\'([0-9\-T:Z]+)\'\s*}}/');

        if ($match === null) {
            return false;
        }

        // Extract the date
        $date = $cursor->getPreviousMatch()[1];

        // Create and add the inline element to the processor
        $inlineContext->getContainer()->appendChild(new DateElement($date));

        return true;
    }
}
