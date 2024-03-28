<?php

use App\AI\GeminiAIGateway;

test('can run inference', function () {
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
