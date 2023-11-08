<?php

use App\Models\Agent;
use App\Models\Step;
use App\Models\Task;
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
    Schema::create('artifacts', function (Blueprint $table) {
      $table->id();
      $table->foreignIdFor(Agent::class)->constrained()->cascadeOnDelete();
      $table->foreignIdFor(Step::class)->cascadeOnDelete()->nullable();
      $table->foreignIdFor(Task::class)->cascadeOnDelete()->nullable();
      $table->string('name')->nullable();
      $table->string('path')->nullable();
      $table->json('data')->nullable();
      $table->timestamps();
    });
  }

  /**
   * Reverse the migrations.
   */
  public function down(): void
  {
    Schema::dropIfExists('artifacts');
  }
};
