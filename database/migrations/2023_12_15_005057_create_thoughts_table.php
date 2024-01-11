<?php

use App\Models\Agent;
use App\Models\Brain;
use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration {
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::create('thoughts', function (Blueprint $table) {
            $table->id();
            $table->foreignIdFor(Agent::class)->cascadeOnDelete()->nullable();
            $table->foreignIdFor(Brain::class)->cascadeOnDelete()->nullable();
            $table->text('body');
            $table->vector('embedding', 768);
            $table->timestamps();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('thoughts');
    }
};
