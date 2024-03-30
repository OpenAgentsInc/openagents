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
        //        'routes/web.php',
        //        'app/AI/GeminiAIGateway.php',
        //        'resources/markdown/launch.md',
        //        'resources/markdown/docs.md',
        //        'resources/markdown/gemini.md',
        //        'resources/markdown/gemini-pro.md',
        //        'resources/markdown/gemini-file-api-faq.md',
        //        'resources/markdown/gemini-file-api-reference.md',
        //        'resources/markdown/20240329-201124-gemini.md',
        //        'tests/Feature/AnalysisTest.php',
        //        'tests/Feature/GeminiTest.php',
        'resources/markdown/20240330-012348-gemini.md',
        'resources/markdown/20240330-012826-gemini.md',
    ];

    $context = CodeAnalyzer::generateContext($filepaths);
    $gemini = new GeminiAIGateway();

    $text = 'Review the conversations below, then select one small component from the attached Figma designs and write code for the Blade or Livewire component, using Tailwind classes.';

    //    $text = 'We are writing a Markdown specification documents describing the attached images in exhaustive detail. Please review the prior entry in this conversation below. Your response was a good start, but does not go into enough detail about the styles you see there. It also does not detail which components are appropriate for Livewire or basic Blade components. Rewrite the document with greater detail.';
    //    $prompt = $text;
    $prompt = $text."\n\n --------- \n\n".$context;
    //    dd($prompt);

    $response = $gemini->inference($prompt, 'new');

    $response = $response['candidates'][0]['content']['parts'][0]['text'];

    dump($response);

    // Write $text to a file with the current timestamp like "20240328-123456-gemini.md" in the resources/markdown folder
    $filename = 'resources/markdown/'.date('Ymd-His').'-gemini.md';

    // Prepend the prompt to the text
    $texttowrite = "# Prompt \n".$text."\n\n # Response \n".$response;

    file_put_contents($filename, $texttowrite);

    //    expect($response)->toBeString();
})->skip();
