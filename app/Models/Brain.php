<?php

namespace App\Models;

use App\Services\QueenbeeGateway;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Brain extends Model
{
    use HasFactory;

    protected $guarded = [];

    public function agent()
    {
        return $this->belongsTo(Agent::class);
    }

    public function datapoints()
    {
        return $this->hasMany(Datapoint::class);
    }

    public function createDatapoint(string $text)
    {
        // For now we'll generate the embedding synchronously, but in the future we'll want to use a queue
        $gateway = new QueenbeeGateway();
        $result = $gateway->createEmbedding($text);
        $embedding = $result[0]['embedding'];

        return $this->datapoints()->create([
            'data' => $text,
            'embedding' => $embedding
        ]);
    }
}
