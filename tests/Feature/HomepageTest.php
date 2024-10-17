<?php

test('homepage loads homepage view', function () {
    $response = $this->get('/');

    $response->assertStatus(200);

    // Assert that view is homepage
    $response->assertViewIs('homepage');
});
