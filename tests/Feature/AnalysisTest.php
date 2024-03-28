<?php

use App\Services\GitHub;

test('can retrieve file contents from github api', function () {
    $github = new GitHub();
    $owner = 'OpenAgentsInc';
    $repo = 'openagents';
    $path = 'README.md';

    $fileDetails = $github->getFileContents($owner, $repo, $path);

    // Assert the structure of the response
    expect($fileDetails)->toBeArray()
        ->toHaveKeys(['name', 'path', 'sha', 'size', 'url', 'html_url', 'git_url', 'download_url', 'type', 'content']);

    // Assert specific fields if necessary
    expect($fileDetails['name'])->toEqual('README.md');
    expect($fileDetails['path'])->toEqual('README.md');
    expect($fileDetails['sha'])->toBeString();
    expect($fileDetails['size'])->toBeInt();
    expect($fileDetails['url'])->toBeString();
    expect($fileDetails['html_url'])->toBeString();
    expect($fileDetails['git_url'])->toBeString();
    expect($fileDetails['download_url'])->toBeString();
    expect($fileDetails['type'])->toEqual('file');

    // Decode and check part of the actual content
    $decodedContent = base64_decode($fileDetails['content']);
    expect($decodedContent)->toContain('OpenAgents');
});
