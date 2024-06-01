<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Run the migrations.
     *
     * @return void
     */
    public function up()
    {
        Schema::table('plugins', function (Blueprint $table) {
            $table->dropColumn('kind');
            $table->dropColumn('mini_template');
            $table->renameColumn('input_template', 'input_sockets');
            $table->renameColumn('plugin_input', 'input_template');
            $table->renameColumn('output_template', 'output_sockets');
        });
    }

    /**
     * Reverse the migrations.
     *
     * @return void
     */
    public function down()
    {
        Schema::table('plugins', function (Blueprint $table) {
            $table->string('kind')->nullable();
            $table->json('mini_template')->nullable();
            $table->renameColumn('input_template', 'plugin_input');
            $table->renameColumn('input_sockets', 'input_template');
            $table->renameColumn('output_sockets', 'output_template');
        });
    }
};
