<?php

namespace App\Services;

class Searcher {
  public function query($file_id, $query) {
    return [
      "ok" => true,
      "results" => "Hello there"
    ];
  }
}
