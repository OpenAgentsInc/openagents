<?php

test('can visit builder page', function () {
    $this->get('/builder')->assertStatus(200);
});
