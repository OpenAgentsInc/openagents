<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('threads', function (Blueprint $table) {
            $table->string('autopilot_id', 36)->nullable()->after('user_id');
            $table->index('autopilot_id', 'threads_autopilot_id_index');
        });

        Schema::table('runs', function (Blueprint $table) {
            $table->string('autopilot_id', 36)->nullable()->after('user_id');
            $table->unsignedInteger('autopilot_config_version')->nullable()->after('autopilot_id');

            $table->index('autopilot_id', 'runs_autopilot_id_index');
        });

        Schema::table('messages', function (Blueprint $table) {
            $table->string('autopilot_id', 36)->nullable()->after('user_id');

            $table->index('autopilot_id', 'messages_autopilot_id_index');
        });

        Schema::table('run_events', function (Blueprint $table) {
            $table->string('autopilot_id', 36)->nullable()->after('user_id');
            $table->string('actor_type', 16)->default('user')->after('autopilot_id');
            $table->string('actor_autopilot_id', 36)->nullable()->after('actor_type');

            $table->index('autopilot_id', 'run_events_autopilot_id_index');
            $table->index('actor_autopilot_id', 'run_events_actor_autopilot_id_index');
            $table->index(['autopilot_id', 'created_at'], 'run_events_autopilot_created_index');
            $table->index(['autopilot_id', 'type', 'id'], 'run_events_autopilot_type_id_index');
            $table->index(['actor_type', 'id'], 'run_events_actor_type_id_index');
            $table->index(['actor_autopilot_id', 'id'], 'run_events_actor_autopilot_id_id_index');
        });
    }

    public function down(): void
    {
        Schema::table('run_events', function (Blueprint $table) {
            $table->dropIndex('run_events_actor_autopilot_id_id_index');
            $table->dropIndex('run_events_actor_type_id_index');
            $table->dropIndex('run_events_autopilot_type_id_index');
            $table->dropIndex('run_events_autopilot_created_index');
            $table->dropIndex('run_events_actor_autopilot_id_index');
            $table->dropIndex('run_events_autopilot_id_index');

            $table->dropColumn(['autopilot_id', 'actor_type', 'actor_autopilot_id']);
        });

        Schema::table('messages', function (Blueprint $table) {
            $table->dropIndex('messages_autopilot_id_index');
            $table->dropColumn('autopilot_id');
        });

        Schema::table('runs', function (Blueprint $table) {
            $table->dropIndex('runs_autopilot_id_index');
            $table->dropColumn(['autopilot_id', 'autopilot_config_version']);
        });

        Schema::table('threads', function (Blueprint $table) {
            $table->dropIndex('threads_autopilot_id_index');
            $table->dropColumn('autopilot_id');
        });
    }
};
