<?php

test('can create embedding from text', function () {

  $embedder = new Embedder();
  $embedding = $embedder->createEmbedding("What is a softbot?");

  // expect embedding to be a 1x768 matrix
  expect($embedding->shape())->toBe([1, 768]);

});
