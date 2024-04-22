<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class PrismSinglePayment extends Model
{
    use HasFactory;

    protected $guarded = [];

    public function multiPayment()
    {
        return $this->belongsTo(PrismMultiPayment::class, 'prism_multi_payment_id');
    }
}
