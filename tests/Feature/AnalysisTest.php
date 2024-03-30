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
        'routes/web.php',
        'app/AI/GeminiAIGateway.php',
        'resources/markdown/launch.md',
        'resources/markdown/docs.md',
        //        'resources/markdown/gemini.md',
        //        'resources/markdown/gemini-pro.md',
        //        'resources/markdown/gemini-file-api-faq.md',
        //        'resources/markdown/gemini-file-api-reference.md',
        'resources/markdown/20240329-201124-gemini.md',
        'tests/Feature/AnalysisTest.php',
        'tests/Feature/GeminiTest.php',
    ];

    $context = CodeAnalyzer::generateContext($filepaths);
    $gemini = new GeminiAIGateway();
    $text = 'We are writing a new Markdown specification describing the attached images in exhaustive detail, with the target audience being a junior developer who will implement the designs in our Laravel codebase. Write a document that will help the developer implement the designs. Focus on each element of the designs, also speculating about what are appropriate sub-components for partial Laravel views and which are appropriate to do as Livewire components vs. basic Blade components.';
    $prompt = $text."\n\n".$context;

    $response = $gemini->inference($prompt, 'new');

    $response = $response['candidates'][0]['content']['parts'][0]['text'];

    dump($response);

    // Write $text to a file with the current timestamp like "20240328-123456-gemini.md" in the resources/markdown folder
    $filename = 'resources/markdown/'.date('Ymd-His').'-gemini.md';

    // Prepend the prompt to the text
    $texttowrite = "# Prompt \n".$text."\n\n # Response \n".$response;

    file_put_contents($filename, $texttowrite);

    //    expect($response)->toBeString();
});
