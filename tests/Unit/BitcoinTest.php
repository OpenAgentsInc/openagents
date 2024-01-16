<?php

use App\Services\Bitcoin;

it('can fetch usd price', function () {
    $price = Bitcoin::getUsdPrice();
    expect($price)->toBeFloat();
});
