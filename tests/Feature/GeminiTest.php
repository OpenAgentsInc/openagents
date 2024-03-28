<?php

use App\AI\GeminiAIGateway;

test('can generate inference', function () {
    $gemini = new GeminiAIGateway();
    $inference = $gemini->inference('Hello, world!');

    // Assert the response structure
    expect($inference)->toBeArray();
    expect($inference)->toHaveKeys(['candidates', 'promptFeedback']);

    // Assert on the first candidate's structure
    $firstCandidate = $inference['candidates'][0];
    expect($firstCandidate)->toBeArray()->toHaveKeys(['content', 'finishReason', 'index', 'safetyRatings']);

    // Assert the structure of 'content' and existence of 'text' within 'parts'
    expect($firstCandidate['content'])->toBeArray()->toHaveKeys(['parts', 'role']);
    expect($firstCandidate['content']['parts'][0])->toBeArray()->toHaveKey('text');
    expect($firstCandidate['content']['role'])->toBeString();

    // Validate 'finishReason' and 'index'
    expect($firstCandidate['finishReason'])->toBeString();
    expect($firstCandidate['index'])->toBeInt();

    // Check the structure of 'safetyRatings'
    expect($firstCandidate['safetyRatings'])->toBeArray()->each(function ($safetyRating) {
        $safetyRating->toBeArray()->toHaveKeys(['category', 'probability']);
        // Ensure 'probability' is one of the expected values
        $safetyRating->probability->toBeIn(['NEGLIGIBLE', 'LOW', 'MEDIUM', 'HIGH']);
    });

    // Validate the structure of 'promptFeedback' and its 'safetyRatings'
    $promptFeedbackSafetyRatings = $inference['promptFeedback']['safetyRatings'];
    expect($promptFeedbackSafetyRatings)->toBeArray()->each(function ($rating) {
        $rating->toBeArray()->toHaveKeys(['category', 'probability']);
        // Ensure 'probability' falls within a known range
        $rating->probability->toBeIn(['NEGLIGIBLE', 'LOW', 'MEDIUM', 'HIGH']);
    });
});

test('can generate chat response', function () {
    $gemini = new GeminiAIGateway();
    $conversation = [
        ['role' => 'user', 'text' => "Pretend you're a snowman and stay in character for each response."],
        ['role' => 'model', 'text' => "Hello! It's so cold! Isn't that great?"],
        ['role' => 'user', 'text' => "What's your favorite season of the year?"],
    ];

    $response = $gemini->chat($conversation);
    //    dump($response);

    // Assert that the response is an array (indicative of a successful structure from the API)
    expect($response)->toBeArray();

    // Depending on the expected structure, you might want to assert that certain keys exist
    // This is a generic check assuming a structure. Adjust according to the actual API response structure
    expect($response)->toHaveKey('candidates');
    $candidates = $response['candidates'];
    expect($candidates)->toBeArray();
    expect($candidates)->not->toBeEmpty();

    // Check the first candidate for a generic structure
    $firstCandidate = $candidates[0];
    expect($firstCandidate)->toHaveKeys(['content', 'finishReason', 'index', 'safetyRatings']);

    // If the API's response includes dynamic values that you can predict or a range of acceptable values,
    // you can insert more specific assertions here to validate those.
});
