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

test('Faerie needs either no params or two params', function () {
    $faerie = new Faerie();
    expect($faerie->owner)->toBe('ArcadeLabsInc');
    expect($faerie->repo)->toBe('openagents');

    $faerie = new Faerie('foo', 'bar');
    expect($faerie->owner)->toBe('foo');
    expect($faerie->repo)->toBe('bar');

    $faerie = new Faerie('foo');
})->throws('Too few arguments to function App\Services\Faerie::__construct(), 1 passed');
