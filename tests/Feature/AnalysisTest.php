<?php

use App\AI\GeminiAIGateway;
use App\Services\CodeAnalyzer;

$skipThese = false;

test('can generate prompt from filepaths', function () {
    $filepaths = [
        'app/Models/User.php',
        'app/Services/GitHub.php',
        'tests/Feature/AnalysisTest.php',
    ];

    $prompt = CodeAnalyzer::generatePrompt($filepaths);

    // Expect prompt to include strings of all those paths, and be greater than 1000 characters
    expect($prompt)->toContain('app/Models/User.php');
    expect($prompt)->toContain('app/Services/GitHub.php');
    expect($prompt)->toContain('tests/Feature/AnalysisTest.php');
    expect(strlen($prompt))->toBeGreaterThan(1000);
})->skip($skipThese);

test('can pass to gemini for analysis', function () {
    $filepaths = CodeAnalyzer::getAllCodebaseFilePaths(base_path()); // first just markdown

    $prompt = CodeAnalyzer::generatePrompt($filepaths);
    $gemini = new GeminiAIGateway();
    $text = "Analyze the following documents. Summarize the project in one paragraph, the API in one paragraph, and recommendations for future directions in a third paragraph. \n".$prompt;
    //    $text = 'Analyze the following code. Write names of feature and unit tests we should write to cover all mentioned functionality. \n '.$prompt;
    $response = $gemini->inference($text, 'new');
    dump($response['candidates'][0]['content']['parts'][0]['text']);

    expect($response)->toBeArray();
    expect($response)->toHaveKey('candidates');
})->skip($skipThese);
