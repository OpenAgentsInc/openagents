<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use App\Traits\RecordsActivity;

class CreditTransaction extends Model
{
    use HasFactory, RecordsActivity;

    protected $fillable = ['team_id', 'user_id', 'amount', 'type', 'description', 'creditable_type', 'creditable_id'];

    protected $casts = [
        'amount' => 'float',
        'created_at' => 'datetime',
        'updated_at' => 'datetime',
    ];

    public function team()
    {
        return $this->belongsTo(Team::class);
    }

    public function user()
    {
        return $this->belongsTo(User::class);
    }

    public function creditable()
    {
        return $this->morphTo();
    }

    public function setAmountAttribute($value)
    {
        $this->attributes['amount'] = round($value, 4);
    }

    public function getAmountAttribute($value)
    {
        return round($value, 4);
    }

    public function setTypeAttribute($value)
    {
        $this->attributes['type'] = in_array($value, ['credit', 'debit', 'purchase', 'usage', 'bonus']) ? $value : 'credit';
    }
}
