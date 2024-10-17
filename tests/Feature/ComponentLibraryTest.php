<?php

test('component library loads component library view', function () {
    $response = $this->get('/components');

    $response->assertStatus(200);

    $response->assertViewIs('components');
});
