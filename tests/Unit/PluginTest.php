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

it('can read what functions a plugin has', function () {
    $plugin = Plugin::factory()->create();
    $functions = $plugin->functions();
    expect($functions)->toBeArray();
    expect($functions)->toContain('count_vowels');
});
