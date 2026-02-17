<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Database\Eloquent\Relations\HasOne;
use Illuminate\Database\Eloquent\SoftDeletes;
use Illuminate\Support\Str;

class Autopilot extends Model
{
    use HasFactory;
    use SoftDeletes;

    protected $table = 'autopilots';

    public $incrementing = false;

    protected $keyType = 'string';

    /**
     * @var list<string>
     */
    protected $fillable = [
        'id',
        'owner_user_id',
        'handle',
        'display_name',
        'avatar',
        'status',
        'visibility',
        'tagline',
        'config_version',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'config_version' => 'integer',
            'created_at' => 'datetime',
            'updated_at' => 'datetime',
            'deleted_at' => 'datetime',
        ];
    }

    protected static function booted(): void
    {
        static::creating(function (self $autopilot): void {
            if (! is_string($autopilot->id) || trim($autopilot->id) === '') {
                $autopilot->id = (string) Str::uuid7();
            }
        });
    }

    public function owner(): BelongsTo
    {
        return $this->belongsTo(User::class, 'owner_user_id');
    }

    public function profile(): HasOne
    {
        return $this->hasOne(AutopilotProfile::class, 'autopilot_id', 'id');
    }

    public function policy(): HasOne
    {
        return $this->hasOne(AutopilotPolicy::class, 'autopilot_id', 'id');
    }

    public function runtimeBindings(): HasMany
    {
        return $this->hasMany(AutopilotRuntimeBinding::class, 'autopilot_id', 'id');
    }

    public function threads(): HasMany
    {
        return $this->hasMany(Thread::class, 'autopilot_id', 'id');
    }

    public function runs(): HasMany
    {
        return $this->hasMany(Run::class, 'autopilot_id', 'id');
    }

    public function messages(): HasMany
    {
        return $this->hasMany(Message::class, 'autopilot_id', 'id');
    }

    public function runEvents(): HasMany
    {
        return $this->hasMany(RunEvent::class, 'autopilot_id', 'id');
    }
}
