<?php

use App\Models\Plugin;

test('plugins page shows list of all plugins', function () {
    $plugin = Plugin::factory(3)->create();
    $response = $this->get('/plugins');
    $response->assertSee($plugin[0]->name);
    $response->assertSee($plugin[1]->name);
    $response->assertSee($plugin[2]->name);

    $response->assertSee($plugin[0]->description);
    $response->assertSee($plugin[1]->description);
    $response->assertSee($plugin[2]->description);

    $response->assertSee($plugin[0]->fee);
    $response->assertSee($plugin[1]->fee);
    $response->assertSee($plugin[2]->fee);

    $response->assertSee($plugin[0]->created_at->format('M d, Y'));
    $response->assertSee($plugin[1]->created_at->format('M d, Y'));
    $response->assertSee($plugin[2]->created_at->format('M d, Y'));
})->group('integration');

test('each plugin links to its show page via id', function () {
    $plugin = Plugin::factory()->create();
    $response = $this->get('/plugins');
    $response->assertSee("/plugin/{$plugin->id}");
})->group('integration');
