<?php

use App\Services\GitHub;

test('can retrieve file contents from github api', function () {
    $github = new GitHub();
    $owner = 'OpenAgentsInc';
    $repo = 'openagents';
    $path = 'README.md';

    $result = $github->getFileContents($owner, $repo, $path);

    // Assert that 'contents' key exists and is a string (the decoded README content)
    expect($result)->toHaveKey('contents');
    expect($result['contents'])->toBeString();
    expect($result['contents'])->toContain('OpenAgents'); // Adjust based on actual content

    // Assert the structure of the full response in 'response' key
    expect($result['response'])->toMatchArray([
        'name' => 'README.md',
        'path' => 'README.md',
        // Continue for other fields as necessary
    ]);

    // If you want to assert the presence of keys without specifying their exact values
    foreach (['sha', 'size', 'url', 'html_url', 'git_url', 'download_url', 'type', 'content'] as $key) {
        expect($result['response'])->toHaveKey($key);
    }
});
