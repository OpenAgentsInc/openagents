<?php

test('can generates thread titles', function () {
    $this->artisan('threads:title')
        ->expectsOutput('Generating thread titles...')
        ->assertExitCode(0);
});
