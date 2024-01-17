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

it('can read what functions a plugin has', function () {
    $plugin = Plugin::factory()->create();
    $functions = $plugin->functions();
    expect($functions)->toBeArray();
    expect($functions)->toContain('count_vowels');
})->skip();

it('can be parsed', function () {
    $plugin = Plugin::factory()->create();
    $plugin->parse();
});
