<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Support\Str;

class AutopilotRuntimeBinding extends Model
{
    use HasFactory;

    protected $table = 'autopilot_runtime_bindings';

    public $incrementing = false;

    protected $keyType = 'string';

    /**
     * @var list<string>
     */
    protected $fillable = [
        'id',
        'autopilot_id',
        'runtime_type',
        'runtime_ref',
        'is_primary',
        'last_seen_at',
        'meta',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'is_primary' => 'boolean',
            'last_seen_at' => 'datetime',
            'meta' => 'array',
            'created_at' => 'datetime',
            'updated_at' => 'datetime',
        ];
    }

    protected static function booted(): void
    {
        static::creating(function (self $binding): void {
            if (! is_string($binding->id) || trim($binding->id) === '') {
                $binding->id = (string) Str::uuid7();
            }
        });
    }

    public function autopilot(): BelongsTo
    {
        return $this->belongsTo(Autopilot::class, 'autopilot_id', 'id');
    }
}
