<?php

/**
 * I’m a developer. I’ve written an agent plugin. Now I want to upload my plugin to OpenAgents and get paid a stream of residual revenue every time my plugin is used in a paid agent flow.
 */

use App\Models\Plugin;

test("user can see upload plugin form", function () {
    $this->get('/plugins')
        ->assertOk()
        ->assertSee('Upload Plugin')
        ->assertViewIs('plugins')
        ->assertSee('upload-plugin');
});

test("user can upload plugin", function () {
    $this->assertEquals(0, count(Plugin::all()));

    $this->post('/plugins', [
        'name' => 'Count Vowels',
        'description' => 'Count the vowels in a string',
        'wasm_url' => "https://github.com/extism/plugins/releases/latest/download/count_vowels.wasm"
    ])
        ->assertOk()
        ->assertSee('Plugin uploaded successfully.');

    $this->assertEquals(1, count(Plugin::all()));
});
