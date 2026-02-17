<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Thread extends Model
{
    use HasFactory;

    protected $table = 'threads';

    public $incrementing = false;

    protected $keyType = 'string';

    /**
     * @var list<string>
     */
    protected $fillable = [
        'id',
        'user_id',
        'autopilot_id',
        'title',
        'created_at',
        'updated_at',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'created_at' => 'datetime',
            'updated_at' => 'datetime',
        ];
    }

    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class, 'user_id');
    }

    public function autopilot(): BelongsTo
    {
        return $this->belongsTo(Autopilot::class, 'autopilot_id', 'id');
    }

    public function runs(): HasMany
    {
        return $this->hasMany(Run::class, 'thread_id', 'id');
    }

    public function messages(): HasMany
    {
        return $this->hasMany(Message::class, 'thread_id', 'id');
    }

    public function runEvents(): HasMany
    {
        return $this->hasMany(RunEvent::class, 'thread_id', 'id');
    }
}
