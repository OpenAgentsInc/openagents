<?php

use App\Models\Agent;
use App\Models\Run;
use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration {
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::create('steps', function (Blueprint $table) {
            $table->id();
            $table->foreignIdFor(Agent::class)->constrained()->cascadeOnDelete();
            $table->string('entry_type');
            $table->string('category');
            $table->string('name');
            $table->string('description')->nullable();
            $table->string('error_message');
            $table->string('success_action');
            $table->json('params')->nullable();
            // $table->foreignIdFor(Run::class)->constrained()->cascadeOnDelete();
            // $table->string('status')->nullable();
            // $table->json('input')->nullable();
            // $table->json('output')->nullable();
            $table->timestamps();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('steps');
    }
};
