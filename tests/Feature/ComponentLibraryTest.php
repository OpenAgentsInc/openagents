<?php

it('returns a successful response', function () {
    $response = $this->get('/components');

    $response->assertStatus(200);
});
