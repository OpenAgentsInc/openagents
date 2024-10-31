<?php

namespace App\Models\CRM;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Company extends Model
{
    /** @use HasFactory<\Database\Factories\CRM\CompanyFactory> */
    use HasFactory;

    protected $fillable = [
        'name',
        'website',
        'industry',
        'description',
    ];

    public function contacts(): HasMany
    {
        return $this->hasMany(Contact::class);
    }
}