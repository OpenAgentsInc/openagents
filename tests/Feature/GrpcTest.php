<?php

use App\Http\Controllers\NostrGrpcController;

test('can connect to nostr service via grpc', function () {
    $controller = new NostrGrpcController();

    dump($controller);
});
