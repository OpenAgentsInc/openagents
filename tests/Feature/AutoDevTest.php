<?php

use App\Agents\AutoDev;

it('works', function () {
    // $autodev = new AutoDev("OpenAgentsInc/openagents");
    $autodev = new AutoDev("ggerganov/llama.cpp");
    $autodev->run();
});

it('requires org and repo', function () {
    $this->expectException(\Exception::class);
    $autodev = new AutoDev("flamp");
    $autodev->run();
});
