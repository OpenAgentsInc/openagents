<?php

use App\Services\Patcher;

test('can clean code block', function () {
    $cleaner = new Patcher();
    $codeBlock = "```\nExample code block\n```";
    $cleanedCodeBlock = $cleaner->cleanCodeBlock($codeBlock);

    expect($cleanedCodeBlock)->toBe("Example code block");
});
