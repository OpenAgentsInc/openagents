<?php

test('component library loads component library view', function () {
    $response = $this->get('/components');

    $response->assertStatus(200);

    // Assert that view is component-library
    $response->assertViewIs('component-library');
});
