<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class RunEvent extends Model
{
    use HasFactory;

    protected $table = 'run_events';

    public $timestamps = false;

    /**
     * @var list<string>
     */
    protected $fillable = [
        'thread_id',
        'run_id',
        'user_id',
        'autopilot_id',
        'actor_type',
        'actor_autopilot_id',
        'type',
        'payload',
        'created_at',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'payload' => 'array',
            'created_at' => 'datetime',
        ];
    }

    public function thread(): BelongsTo
    {
        return $this->belongsTo(Thread::class, 'thread_id', 'id');
    }

    public function run(): BelongsTo
    {
        return $this->belongsTo(Run::class, 'run_id', 'id');
    }

    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class, 'user_id');
    }

    public function autopilot(): BelongsTo
    {
        return $this->belongsTo(Autopilot::class, 'autopilot_id', 'id');
    }

    public function actorAutopilot(): BelongsTo
    {
        return $this->belongsTo(Autopilot::class, 'actor_autopilot_id', 'id');
    }
}
