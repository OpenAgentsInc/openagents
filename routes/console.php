<?php

use Illuminate\Support\Facades\Schedule;

Schedule::command('telescope:prune --hours=48')->daily();
Schedule::command('report:usage')->daily();
Schedule::command('threads:title')->everyMinute();
Schedule::command('payins:check')->everyMinute();
Schedule::command('sweep')->everyMinute();
Schedule::command('plugins:short_desc')->everyMinute();
