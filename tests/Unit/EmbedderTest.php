<?php

use App\Services\Embedder;

it('can generate a fake embedding', function () {
    $embedder = Embedder::createFakeEmbedding();
    expect(count($embedder))->toBe(768);
});
