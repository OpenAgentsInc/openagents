<?php

/**
 * I’m a developer. I’ve written an agent plugin. Now I want to upload my plugin to OpenAgents and get paid a stream of residual revenue every time my plugin is used in a paid agent flow.
 */

use App\Models\Plugin;

test("user can see upload plugin form", function () {
    $this->get('/plugins/create')
        ->assertOk()
        ->assertSee('Upload Plugin')
        ->assertViewIs('plugin-create')
        ->assertSee('upload-plugin');
});

test("user can upload plugin", function () {
    $this->assertEquals(0, count(Plugin::all()));

    $this->post('/plugins', [
        'name' => 'Count Vowels',
        'fee' => '100',
        'description' => 'Count the vowels in a string',
        'wasm_url' => "https://github.com/extism/plugins/releases/latest/download/count_vowels.wasm"
    ])
        ->assertOk();
    // ->assertSee('Plugin uploaded successfully.'); -- javascript doh!

    $this->assertEquals(1, count(Plugin::all()));
});

test("upload requires plugin to have a name", function () {
    $this->assertEquals(0, count(Plugin::all()));

    $this->post('/plugins', [
        'fee' => 0,
        'description' => 'Count the vowels in a string',
        'wasm_url' => "http://theurl.com/count_vowels.wasm"
    ])
        ->assertStatus(200)
        ->assertSee('The name field is required.');
});

test("upload requires plugin to have a description", function () {
    $this->assertEquals(0, count(Plugin::all()));

    $this->post('/plugins', [
        'fee' => 0,
        'name' => 'Count Vowels',
        'wasm_url' => "http://theurl.com/count_vowels.wasm"
    ])
        ->assertStatus(200)
        ->assertSee('The description field is required.');
});

test('upload requires plugin to have a wasm_url', function () {
    $this->assertEquals(0, count(Plugin::all()));

    $this->post('/plugins', [
        'name' => 'Count Vowels',
        'description' => 'Count the vowels in a string',
    ])
        ->assertStatus(200)
        ->assertSee('The wasm url field is required.');
});

test('wasm_url must be an actual url', function () {
    $this->assertEquals(0, count(Plugin::all()));

    $this->post('/plugins', [
        'name' => 'Count Vowels',
        'description' => 'Count the vowels in a string',
        'wasm_url' => "not a url"
    ])
        ->assertStatus(200)
        ->assertSee('The wasm url field must be a valid URL.');
});
