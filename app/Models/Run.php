<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class Run extends Model
{
    use HasFactory;

    // belongs to a flow
    public function flow(): BelongsTo
    {
        return $this->belongsTo(Flow::class);
    }
}
