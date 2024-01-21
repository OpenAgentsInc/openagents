<?php

namespace App\Markdown;

use League\CommonMark\Node\Inline\AbstractInline;

class DateElement extends AbstractInline
{
    private string $date;

    public function __construct(string $date)
    {
        parent::__construct();

        $this->date = $date;
    }

    public function getDate(): string
    {
        return $this->date;
    }
}
