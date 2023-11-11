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

  $corpusData = [
    'name' => 'Test Corpus',
    'description' => 'This is a test corpus',
  ];

  $response = $vectara->createCorpus(1, $corpusData);
  expect($response['ok'])->toBeTrue();

});

// test('can upload file to vectara', function () {
//   $file = UploadedFile::fake()->create('document.pdf', 1000);

//   $vectara = new Vectara();
//   $uploadResponse = $vectara->upload($file);

//   expect($uploadResponse['ok'])->toBeTrue();
// });
