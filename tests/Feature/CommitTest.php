<?php

use App\Services\Patcher;

test('can clean code block', function () {
    $cleaner = new Patcher();
    $codeBlock = "```\nExample code block\n```";
    $cleanedCodeBlock = $cleaner->cleanCodeBlock($codeBlock);

    expect($cleanedCodeBlock)->toBe("Example code block");
});


test('can get issue patches', function () {
    $patcher = new Patcher();
    $issue = [
        "title" => "Capitalize all comments",
        "body" => "Make sure all comments are IN ALL CAPS.",
    ];
    $patches = $patcher->getIssuePatches($issue, 2);

    expect($patches)->toBeArray();
    expect($patches)->toHaveCount(2);
    expect($patches[0])->toHaveKey('file_name');
    expect($patches[0])->toHaveKey('content');
    expect($patches[0])->toHaveKey('new_content');
});

test('can submit issue patches to github', function () {
  $patcher = new Patcher();
  $issue = [
      "title" => "Capitalize all comments",
      "body" => "Make sure all comments (lines beginning with //) are IN ALL CAPS.",
  ];
  $patches = $patcher->getIssuePatches($issue, 3);
  $res = $patcher->submitPatchesToGitHub($patches, "ArcadeLabsInc/trashheap", "testbranch");

  expect($res)->toBeArray();
  expect($res["ok"])->toBeTrue();
});
