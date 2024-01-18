<?php

use App\Models\Plugin;

it('has a name', function () {
    $plugin = Plugin::factory()->create([
        'name' => 'Count Vowels'
    ]);
    expect($plugin->name)->toBe('Count Vowels');
});

it('has a wasm_url', function () {
    $plugin = Plugin::factory()->create([
        'wasm_url' => 'https://github.com/extism/plugins/releases/latest/download/count_vowels.wasm'
    ]);
    expect($plugin->wasm_url)->toBe('https://github.com/extism/plugins/releases/latest/download/count_vowels.wasm');
});

test('can execute plugin function', function () {
    $plugin = Plugin::factory()->create();
    $output = $plugin->call("count_vowels", "Yellow, World!");
    expect($output)->toBe('{"count":3,"total":3,"vowels":"aeiouAEIOU"}');
});

test('can return its module functions', function () {
    $plugin = Plugin::factory()->create([
        'name' => 'Count Vowels',
        'wasm_url' => 'https://github.com/extism/plugins/releases/latest/download/count_vowels.wasm',
    ]);
    $functions = $plugin->functions();
    expect($functions)->toBeArray();
    expect($functions)->toContain('count_vowels');
});

it('can be parsed', function () {
    $plugin = Plugin::factory()->create();
    $parsed = $plugin->parse();
    expect($parsed)->toBeArray();
    expect($parsed["module_hash"])->toBeString();
    expect($parsed["module_hash"])->toBe('93898457953d30d016f712ccf4336ce7e9971db5f7f3aff1edd252764f75d5d7');
});
