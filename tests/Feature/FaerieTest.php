<?php

use App\Models\Step;
use App\Models\User;
use App\Services\Faerie;

beforeEach(function () {
    $this->actingAs(User::factory()->create());
});

/**
 * INSTANTIATION
 */

it('defaults to the openagents repo', function () {
    $faerie = new Faerie();
    expect($faerie->owner)->toBe('OpenAgentsInc');
    expect($faerie->repo)->toBe('openagents');
});

it('can be instantiated with a different repo', function () {
    $faerie = new Faerie('foo', 'bar');
    expect($faerie->owner)->toBe('foo');
    expect($faerie->repo)->toBe('bar');
});

/**
 * RECORD STEPS
 */
it('can record a step', function () {
    $faerie = new Faerie();
    $response = $faerie->recordStep('foo', 'bar', 'baz');
    expect($response)->toBeArray();
    expect($response['status'])->toBe('success');

    // expect response step to be an instance of Step
    expect($response['step'])->toBeInstanceOf(Step::class);

    expect($response['step']['description'])->toBe('foo');
    expect(json_decode($response['step']['input']))->toBe('bar');
    expect(json_decode($response['step']['output']))->toBe('baz');
})->skip();

/**
 * READ GITHUB REPO
 */

it('can determine if repo has an open PR', function () {
    $faerie = new Faerie();
    $response = $faerie->repoHasOpenPR();
    expect($response)->toBeBool();
})->group('faerie');

it('can fetch the most recent issue', function () {
    $faerie = new Faerie();
    $response = $faerie->fetchMostRecentIssue();
    expect($response)->toBeArray();
    expect($response['title'])->toBeString();
})->group('faerie');

it('can fetch the most recent PR', function () {
    $faerie = new Faerie();
    $response = $faerie->fetchMostRecentPR();
    expect($response)->toBeArray();
    expect($response['title'])->toBeString();
})->group('faerie');

/**
 * ANALYZE GITHUB REPO
 */
it('can analyze a PR', function () {
    $faerie = new Faerie();
    $faerie->fetchMostRecentPR();
    $response = $faerie->analyzePr();
    // print_r($response);
    expect($response)->toBeArray();
    expect($response['status'])->toBe('success');
    expect($response['comment'])->toBeString();
})->group('faerie');

/**
 * HAPPY PATH
 */

it('can run a task', function () {
    expect(Step::count())->toBe(0);

    $faerie = new Faerie();
    $response = $faerie->run();

    expect($response)->toBeArray();
    expect($response['status'])->toBe('success');
    expect(Step::count())->toBeGreaterThan(0);
})->group('faerie');
