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
        'resources/markdown/launch.md',
        'tests/Feature/AnalysisTest.php',
        'tests/Feature/GeminiTest.php',
        'tests/Feature/GitHubTest.php',
        'resources/markdown/20240328-gemini.md',
        'resources/markdown/20240329-024442-gemini.md',
        'resources/markdown/20240329-024920-gemini.md',
    ];

    $prompt = CodeAnalyzer::generatePrompt($filepaths);
    $gemini = new GeminiAIGateway();
    $text = "Continue the conversation based on 20240329-024442-gemini.md. You gave a partial code snippet. I need you to write the entire contents of the file so I can copy-paste it in entirely. Ensure that the tests in GeminiTest will continue to pass. \n".$prompt;
    //    $text = 'Analyze the following code. Write names of feature and unit tests we should write to cover all mentioned functionality. \n '.$prompt;
    $response = $gemini->inference($text, 'new');

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
