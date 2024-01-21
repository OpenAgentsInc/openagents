<?php

use App\Models\Plugin;

test('can call a plugin', function () {
    $plugin = Plugin::factory()->create();
    $response = $this->post(route('plugins.call'), [
        'plugin_id' => $plugin->id,
        'function' => 'count_vowels',
        'input' => 'Yellow, World!',
    ]);
    $response->assertStatus(200);
    $response->assertSee('6');
})->group('integration');
