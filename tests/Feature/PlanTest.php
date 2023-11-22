<?php

use App\Services\Planner;

test('can create plan based on github conversation', function () {
    // Given a GitHub issue and conversation
    $response = GitHub::issues()->show('ArcadeLabsInc', 'openagents', 1);
    $commentsResponse = GitHub::api('issue')->comments()->all('ArcadeLabsInc', 'openagents', 1);
    $body = $response['body'];
    $title = $response['title'];

    // When I create a plan from the conversation
    $planner = new Planner();
    $messages = $planner->formatIssueAndCommentsAsMessages($body, $commentsResponse);
    $plan = $planner->createPlan($messages);

    // The plan should be a single string
    // print_r($plan);
    expect($plan)->toBeString();
});
