<?php

use App\AI\Models;

it('can verify whether user has access to a model', function () {
    expect(Models::hasModelAccess('gpt-3.5-turbo-16k', 'user'))->toBeTrue()
        ->and(Models::hasModelAccess('gpt-4', 'user'))->toBeFalse()
        ->and(Models::hasModelAccess('unknown', 'user'))->toBeFalse();
});

it('can distinguish a pro model from a non-pro model', function () {
    expect(Models::isProModelSelected('gpt-4'))->toBeTrue()
        ->and(Models::isProModelSelected('gpt-3.5-turbo-16k'))->toBeFalse()
        ->and(Models::isProModelSelected('unknown'))->toBeFalse();
});

it('returns models for pro users', function () {
    $userTypes = ['pro'];
    $models = Models::getModelsForUserTypes($userTypes);

    foreach ($models as $model) {
        expect(Models::isProModelSelected($model))->toBeTrue();
    }
});

it('returns non-pro models for non-pro users', function () {
    $userTypes = ['guest', 'user'];
    $models = Models::getModelsForUserTypes($userTypes);

    foreach ($models as $model) {
        expect(Models::isProModelSelected($model))->toBeFalse();
    }
});

it('gets non-pro models for non-pro users selector', function () {
    $userTypes = ['guest', 'user'];
    $models = Models::getSelectModelsForUserTypes($userTypes);

    foreach ($models as $model => $name) {
        expect(Models::isProModelSelected($model))->toBeFalse();
    }
});
