<?php

use App\Http\Controllers\NostrGrpcController;

test('can connect to nostr service via grpc', function () {
    $controller = new NostrGrpcController();
    $res = $controller->requestContext('oa.forkforge.net:5000', 'Testing');

    dump($res);
});
