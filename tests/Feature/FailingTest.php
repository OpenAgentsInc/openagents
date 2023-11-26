<?php

test('this test should fail', function () {
    expect(true)->toBeFalse();
})->skip();

test('and this test too', function () {
  expect(true)->toBeFalse();
})->skip();
