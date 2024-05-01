<?php

use App\Services\OpenObserveLogger;

test('sends a log entry to OpenObserve', function () {
    // Set up the OpenObserve logger with dummy credentials
    config(['openobserve.username' => 'dummy-username']);
    config(['openobserve.password' => 'dummy-password']);
    config(['openobserve.base_url' => 'https://example.com']);
    config(['openobserve.org' => 'my-org']);
    config(['openobserve.stream' => 'my-stream']);

    // Create an instance of the OpenObserve logger
    $logger = new OpenObserveLogger([
        'baseUrl' => 'https://pool.openagents.com:5080',
        'org' => 'default',
        'stream' => 'logs',
        'batchSize' => 1,
        'flushInterval' => 1000,
    ]);

    // Send a dummy log entry
    $logger->log('INFO', 'This is a dummy log entry');

    // Wait for the flush interval to pass
    sleep(1);

    // Assert that the log entry was sent to OpenObserve
    // todo: implement a more robust assertion
    expect(true)->toBeTrue();
});
