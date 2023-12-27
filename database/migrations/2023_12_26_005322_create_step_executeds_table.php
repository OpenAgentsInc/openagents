<?php

use App\Models\Step;
use App\Models\TaskExecuted;
use App\Models\User;
use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration {
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::create('step_executeds', function (Blueprint $table) {
            $table->id();
            $table->foreignIdFor(Step::class)->constrained()->cascadeOnDelete();
            $table->foreignIdFor(TaskExecuted::class)->constrained()->cascadeOnDelete();
            $table->foreignIdFor(User::class)->nullable();
            $table->string('status');
            $table->integer('order');
            $table->json('input')->nullable();
            $table->json('output')->nullable();
            $table->json('params')->nullable();
            // TODO: polymorphic ref
            $table->timestamps();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('step_executeds');
    }
};
