<?php

namespace App\Traits;

use App\Models\Activity;

trait RecordsActivity
{
    protected static function bootRecordsActivity()
    {
        static::created(function ($model) {
            $model->recordActivity('created');
        });

        static::updated(function ($model) {
            $model->recordActivity('updated');
        });

        static::deleted(function ($model) {
            $model->recordActivity('deleted');
        });
    }

    public function recordActivity($verb)
    {
        $user_id = auth()->id() ?? $this->user_id ?? null;
        $team_id = $this->team_id ?? null;

        // If the model has a user_id but no team_id, use that user_id
        if (!$team_id && isset($this->user_id)) {
            $user_id = $this->user_id;
        }

        // If the model has a team() relationship, use that team's id
        if (method_exists($this, 'team') && $this->team) {
            $team_id = $this->team->id;
        }

        Activity::create([
            'user_id' => $user_id,
            'team_id' => $team_id,
            'verb' => $verb,
            'subject_type' => get_class($this),
            'subject_id' => $this->id,
        ]);
    }
}