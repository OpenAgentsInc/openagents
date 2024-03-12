<?php

use App\Services\OpenAIGateway;

test('can comment on pr conversation', function () {

    $owner = 'OpenAgentsInc';
    $repository = 'openagents';
    $pullRequestNumber = 14;

    $response = GitHub::issues()->show($owner, $repository, $pullRequestNumber);
    $title = $response['title'];

    $system = 'You are Faerie, an AI agent specialized in writing & analyzing code.

You have been summoned to OpenAgentsInc/openagents pull request.

The pull request is titled `'.$title."`

Please respond with the comment you would like to add based on the comments. If there are any failing tests, suggest how to fix them. Write like a senior developer would write; don't introduce yourself or use flowery text or a closing signature.";

    $comments = GitHub::api('issue')->comments()->all($owner, $repository, $pullRequestNumber);

    // For each comment, add it to the messages array with role user
    $messages = [];
    foreach ($comments as $comment) {
        $messages[] = ['role' => 'user', 'content' => $comment['body']];
    }

    $gateway = new OpenAIGateway();

    $response = $gateway->makeChatCompletion([
        'model' => 'gpt-4',
        'messages' => [
            ['role' => 'system', 'content' => $system],
            ...$messages,
        ],
    ]);

    $comment = $response['choices'][0]['message']['content'];
    print_r($comment);
    expect($comment)->toBeString();

    // GitHub::api('issue')->comments()->create($owner, $repository, $pullRequestNumber, array('body' => $comment));
})->group('integration');

test('can pull runs from pr', function () {
    $owner = 'OpenAgentsInc';
    $repo = 'openagents';
    $reference = 'testpestpr';
    $checks = GitHub::api('repo')->checkRuns()->allForReference($owner, $repo, $reference);
    expect($checks['total_count'])->toBeInt();
})->group('integration');

test('can create and delete github branch', function () {
    $owner = 'OpenAgentsInc';
    $repo = 'openagents';
    $baseBranch = 'main';
    $commit = GitHub::api('repo')->commits()->all($owner, $repo, ['sha' => $baseBranch])[0];
    $latestCommitSha = $commit['sha'];

    // Step 2: Create the new branch
    $newBranch = 'delete_me';
    $response = GitHub::api('git')->references()->create($owner, $repo, [
        'ref' => 'refs/heads/'.$newBranch,
        'sha' => $latestCommitSha,
    ]);
    expect($response['ref'])->toBe('refs/heads/'.$newBranch);

    // Step 3: Delete the branch
    $response = GitHub::api('git')->references()->remove($owner, $repo, 'heads/'.$newBranch);
    expect($response)->toBe('');
})->group('integration');

test('can fetch github issue', function () {
    $response = GitHub::issues()->show('OpenAgentsInc', 'openagents', 1);

    expect($response['url'])->toBe('https://api.github.com/repos/OpenAgentsInc/openagents/issues/1');
    expect($response['repository_url'])->toBe('https://api.github.com/repos/OpenAgentsInc/openagents');
    expect($response['body'])->toContain("We will implement the 'memory stream' architecture mentioned in the Generative Agents paper, excerpted below and slightly modified to reflect our 'autodev' use case.");
})->group('integration');

test('can fetch github issue comments', function () {
    // $response = GitHub::api('issue')->comments()->show('OpenAgentsInc', 'openagents', 1);
    $response = GitHub::api('issue')->comments()->all('OpenAgentsInc', 'openagents', 1);

    // Test for the first comment
    expect($response[0]['url'])->toBe('https://api.github.com/repos/OpenAgentsInc/openagents/issues/comments/1817537867');
    expect($response[0]['html_url'])->toBe('https://github.com/OpenAgentsInc/openagents/issues/1#issuecomment-1817537867');
    expect($response[0]['issue_url'])->toBe('https://api.github.com/repos/OpenAgentsInc/openagents/issues/1');
    expect($response[0]['user']['login'])->toBe('FaerieAI');
    expect($response[0]['body'])->toContain('This `memory stream` structure, as described, is quite intriguing and appears well-suited for our needs.');

    // Test for the second comment
    expect($response[1]['url'])->toBe('https://api.github.com/repos/OpenAgentsInc/openagents/issues/comments/1817553619');
    expect($response[1]['html_url'])->toBe('https://github.com/OpenAgentsInc/openagents/issues/1#issuecomment-1817553619');
    expect($response[1]['issue_url'])->toBe('https://api.github.com/repos/OpenAgentsInc/openagents/issues/1');
    expect($response[1]['user']['login'])->toBe('AtlantisPleb');
    expect($response[1]['body'])->toContain('Thank you @FaerieAI, that was a good initial answer.');
})->group('integration');

test('can respond to github issue as faerie', function () {
    $response = GitHub::issues()->show('OpenAgentsInc', 'openagents', 1);

    $body = $response['body'];
    $title = $response['title'];

    $prompt = 'You are Faerie, an AI agent specialized in writing & analyzing code.

You have been summoned to OpenAgentsInc/openagents issue #1.

The issue is titled `'.$title.'`

The issue body is:
```
'.$body."
```

Please respond with the comment you would like to add to the issue. Write like a senior developer would write; don't introduce yourself or use flowery text or a closing signature.";

    $gateway = new OpenAIGateway();

    $response = $gateway->makeChatCompletion([
        'model' => 'gpt-4',
        'messages' => [
            // ['role' => 'system', 'content' => 'You are a helpful assistant.'],
            ['role' => 'user', 'content' => $prompt],
        ],
    ]);

    $this->assertIsArray($response);
    $this->assertIsString($response['id']);
    $this->assertIsString($response['object']);
    $this->assertIsString($response['model']);
    $this->assertIsArray($response['choices']);
    $this->assertIsArray($response['usage']);
    $this->assertIsInt($response['created']);
})->group('integration');
