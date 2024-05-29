<?php

test('explorer route loads', function () {
    $this->get('/explorer')
        ->assertStatus(200)
        ->assertSee('Explorer');
});
