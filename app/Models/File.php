<?php

namespace App\Models;

use App\Services\QueenbeeGateway;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class File extends Model
{
    use HasFactory;

    protected $guarded = [];

    public function embeddings()
    {
        return $this->hasMany(Embedding::class);
    }

    // belongs to a user
    public function user()
    {
        return $this->belongsTo(User::class);
    }

    public function createEmbeddings()
    {
        if (is_file($this->path) === false) {
            return;
        }
        // Open the file and read its text contents into a variable
        $text = file_get_contents($this->path);

        $gateway = new QueenbeeGateway();
        $result = $gateway->createEmbedding($text);
        $embedding = $result[0]['embedding'];

        $this->embeddings()->create([
            'embedding' => $embedding,
            'metadata' => [
                'path' => $this->path,
            ],
        ]);
    }
}
