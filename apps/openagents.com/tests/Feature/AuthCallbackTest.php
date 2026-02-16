<?php

test('authenticate callback without code and state redirects to login', function () {
    $this->get('/authenticate')->assertRedirect('/login');
});
