<?php

use App\Models\Plugin;

test('can view plugin show page', function () {
    $plugin = Plugin::factory()->create();
    $response = $this->get(route('plugins.show', $plugin));
    $response->assertStatus(200);
});

test('plugin show page shows plugin name, description, fee and wasm_url', function () {
    $plugin = Plugin::factory()->create();
    $response = $this->get(route('plugins.show', $plugin));
    $response->assertSee($plugin->name);
    $response->assertSee($plugin->description);
    $response->assertSee($plugin->fee);
    $response->assertSee($plugin->wasm_url);
});

test('if plugin not found, redirect to home page', function () {
    $response = $this->get(route('plugins.show', 1));
    $response->assertRedirect("/");
});
