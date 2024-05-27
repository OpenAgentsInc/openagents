<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::create('nostr_jobs', function (Blueprint $table) {
            $table->id();
            $table->foreignId('agent_id')->nullable();
            $table->bigInteger('thread_id')->nullable();
            $table->text('job_id')->nullable();
            $table->json('payload')->nullable();
            $table->longText('content')->nullable();
            $table->char('status', 25)->nullable();
            $table->boolean('warmup')->default(false);
            $table->timestamps();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('nostr_jobs');
    }
};
