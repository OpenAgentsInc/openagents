<?php

use App\Models\Plugin;

test('plugins page shows list of all plugins', function () {
    $plugin = Plugin::factory(3)->create();
    $response = $this->get('/plugins');
    $response->assertSee($plugin[0]->name);
    $response->assertSee($plugin[1]->name);
    $response->assertSee($plugin[2]->name);
});
