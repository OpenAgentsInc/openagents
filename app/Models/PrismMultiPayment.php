<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class PrismMultiPayment extends Model
{
    use HasFactory;

    protected $guarded = [];

    public function singlePayments()
    {
        return $this->hasMany(PrismSinglePayment::class, 'prism_multi_payment_id');
    }
}
