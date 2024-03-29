<?php

use App\AI\GeminiAIGateway;
use App\Services\CodeAnalyzer;

$skipThese = true;

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
    //    $filepaths = CodeAnalyzer::getAllCodebaseFilePaths(base_path()); // first just markdown
    $filepaths = [
        'app/AI/GeminiAIGateway.php',
        'resources/markdown/docs.md',
        'resources/markdown/gemini.md',
        'resources/markdown/gemini-file-api-faq.md',
        'resources/markdown/gemini-file-api-reference.md',
        'resources/markdown/gemini-pro.md',
        'resources/markdown/20240329-161749-gemini.md',
        'tests/Feature/AnalysisTest.php',
        'tests/Feature/GeminiTest.php',
        'tests/Feature/GitHubTest.php',
    ];

    $context = CodeAnalyzer::generateContext($filepaths);
    $gemini = new GeminiAIGateway();
    $text = 'Write the GeminiAIGateway uploadFile method based on the File API documentation.';
    $prompt = $text."\n\n".$context;

    $response = $gemini->inference($prompt, 'new');

    $response = $response['candidates'][0]['content']['parts'][0]['text'];

    dump($response);

    // Write $text to a file with the current timestamp like "20240328-123456-gemini.md" in the resources/markdown folder
    $filename = 'resources/markdown/'.date('Ymd-His').'-gemini.md';

    // Prepend the prompt to the text
    $texttowrite = "# Prompt \n".$text."\n\n".$response;

    file_put_contents($filename, $texttowrite);

    expect($response)->toBeArray();
    expect($response)->toHaveKey('candidates');
});
