<?php

namespace App\Services;

class QueenbeeGateway {
  public function createEmbedding() {
    $embedding = [];
    for ($i = 0; $i < 768; $i++) {
      $embedding[] = rand(0, 1); // or any logic to generate your embedding values
    }
    return $embedding;
  }
}
