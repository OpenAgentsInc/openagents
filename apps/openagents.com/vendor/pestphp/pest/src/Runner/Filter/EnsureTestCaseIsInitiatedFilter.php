<?php

declare(strict_types=1);

namespace Pest\Runner\Filter;

use Pest\Contracts\HasPrintableTestCaseName;
use PHPUnit\Framework\Test;
use RecursiveFilterIterator;
use RecursiveIterator;

/**
 * @internal
 */
final class EnsureTestCaseIsInitiatedFilter extends RecursiveFilterIterator
{
    /**
     * @param  RecursiveIterator<int, Test>  $iterator
     */
    public function __construct(RecursiveIterator $iterator)
    {
        parent::__construct($iterator);
    }

    /**
     * {@inheritdoc}
     */
    public function accept(): bool
    {
        $test = $this->getInnerIterator()->current();

        if ($test instanceof HasPrintableTestCaseName) {
            /** @phpstan-ignore-next-line */
            $test->__initializeTestCase();
        }

        return true;
    }
}
