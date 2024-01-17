<?php

use App\Models\Plugin;

test('can read what functions a plugin has', function () {
    $plugin = Plugin::factory()->create([
        'name' => 'Count Vowels',
        'wasm' => 'https://github.com/extism/plugins/releases/latest/download/count_vowels.wasm',
    ]);
    $functions = $plugin->functions();
    expect($functions)->toBeArray();
    expect($functions)->toContain('count_vowels');
});
