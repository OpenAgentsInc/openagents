<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class AutopilotPolicy extends Model
{
    use HasFactory;

    protected $table = 'autopilot_policies';

    protected $primaryKey = 'autopilot_id';

    public $incrementing = false;

    protected $keyType = 'string';

    /**
     * @var list<string>
     */
    protected $fillable = [
        'autopilot_id',
        'model_provider',
        'model',
        'tool_allowlist',
        'tool_denylist',
        'l402_require_approval',
        'l402_max_spend_msats_per_call',
        'l402_max_spend_msats_per_day',
        'l402_allowed_hosts',
        'data_policy',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'tool_allowlist' => 'array',
            'tool_denylist' => 'array',
            'l402_require_approval' => 'boolean',
            'l402_max_spend_msats_per_call' => 'integer',
            'l402_max_spend_msats_per_day' => 'integer',
            'l402_allowed_hosts' => 'array',
            'data_policy' => 'array',
            'created_at' => 'datetime',
            'updated_at' => 'datetime',
        ];
    }

    public function autopilot(): BelongsTo
    {
        return $this->belongsTo(Autopilot::class, 'autopilot_id', 'id');
    }
}
