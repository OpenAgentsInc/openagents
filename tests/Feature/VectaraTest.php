<?php

use App\Services\Vectara;
use Illuminate\Http\UploadedFile;

test('can get jwt', function () {
  $vectara = new Vectara();
  $jwtToken = $vectara->getJwtToken();

  expect($jwtToken)->not->toBeNull();
});

test('can create corpus', function () {
  $vectara = new Vectara();
  $jwtToken = $vectara->getJwtToken();

  $response = $vectara->createCorpus([
    'name' => 'Test Corpus',
    'description' => 'This is a test corpus',
  ]);
  expect($response['ok'])->toBeTrue();

});

test('can upload file to corpus', function () {
  $file = new UploadedFile(
    storage_path('app/uploads/0sYtEObUcMCnLo8zAwv7i0cJPfoWTPO4tW7ZblS0.pdf'),
    '0sYtEObUcMCnLo8zAwv7i0cJPfoWTPO4tW7ZblS0.pdf',
    'application/pdf',
    null,
    true
  );

  $vectara = new Vectara();
  $uploadResponse = $vectara->upload(5, $file);

  expect($uploadResponse['ok'])->toBeTrue();
});
