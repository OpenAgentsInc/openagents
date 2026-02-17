<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class AutopilotProfile extends Model
{
    use HasFactory;

    protected $table = 'autopilot_profiles';

    protected $primaryKey = 'autopilot_id';

    public $incrementing = false;

    protected $keyType = 'string';

    /**
     * @var list<string>
     */
    protected $fillable = [
        'autopilot_id',
        'owner_display_name',
        'persona_summary',
        'autopilot_voice',
        'principles',
        'preferences',
        'onboarding_answers',
        'schema_version',
    ];

    /**
     * @return array<string, string>
     */
    protected function casts(): array
    {
        return [
            'principles' => 'array',
            'preferences' => 'array',
            'onboarding_answers' => 'array',
            'schema_version' => 'integer',
            'created_at' => 'datetime',
            'updated_at' => 'datetime',
        ];
    }

    public function autopilot(): BelongsTo
    {
        return $this->belongsTo(Autopilot::class, 'autopilot_id', 'id');
    }
}
