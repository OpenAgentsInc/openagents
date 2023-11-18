<?php

use App\Services\QueenbeeGateway;

test('can fetch github issue', function () {

  dd(GitHub::repo()->show('ArcadeLabsInc', 'openagents'));

});
