<?php

use App\Services\Faerie;

/**
 * INSTANTIATION
 */

it('defaults to the openagents repo', function () {
    $faerie = new Faerie();
    expect($faerie->owner)->toBe('ArcadeLabsInc');
    expect($faerie->repo)->toBe('openagents');
});

it('can be instantiated with a different repo', function () {
    $faerie = new Faerie('foo', 'bar');
    expect($faerie->owner)->toBe('foo');
    expect($faerie->repo)->toBe('bar');
});

/**
 * READ GITHUB REPO
 */

it('can determine if repo has an open PR', function () {
    $faerie = new Faerie();
    $response = $faerie->repoHasOpenPR();
    expect($response)->toBeBool();
});

// it('can read a github repo', function () {
//     $faerie = new Faerie();
//     $response = $faerie->readRepo();
//     expect($response)->toBeArray();
//     expect($response['name'])->toBe('openagents');
// });
