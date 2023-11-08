<?php

it('has many agents', function () {

  $user = App\Models\User::factory()->create();

  $this->assertInstanceOf(
    'Illuminate\Database\Eloquent\Collection',
    $user->agents
  );

});
