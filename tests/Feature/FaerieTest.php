<?php

use App\Services\Faerie;

/**
 * INSTANTIATION
 */

test('Faerie defaults to the openagents repo', function () {
    $faerie = new Faerie();
    expect($faerie->owner)->toBe('ArcadeLabsInc');
    expect($faerie->repo)->toBe('openagents');
});

test('Faerie can be instantiated with a different repo', function () {
    $faerie = new Faerie('foo', 'bar');
    expect($faerie->owner)->toBe('foo');
    expect($faerie->repo)->toBe('bar');
});
