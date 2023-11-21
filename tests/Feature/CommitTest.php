<?php

use App\Services\Patcher;

test('can clean code block', function () {
    $cleaner = new Patcher();
    $codeBlock = "```\nExample code block\n```";
    $cleanedCodeBlock = $cleaner->cleanCodeBlock($codeBlock);

    expect($cleanedCodeBlock)->toBe("Example code block");
});


test('can get issue patches', function () {
    $cleaner = new Patcher();
    $issue = [
        "title" => "Example issue",
        "body" => "Example issue body",
    ];
    $patches = $cleaner->getIssuePatches($issue);

    // expect($patches)->toBeArray();
    // expect($patches)->toHaveCount(1);
    // expect($patches[0])->toHaveKey('file_name');
    // expect($patches[0])->toHaveKey('content');
    // expect($patches[0])->toHaveKey('new_content');
});
