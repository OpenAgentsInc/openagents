<?php

test('visiting start page returns successful response', function () {
  $response = $this->get('/start');

  $response->assertStatus(200);
});
