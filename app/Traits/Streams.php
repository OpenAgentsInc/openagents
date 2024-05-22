<?php

namespace App\Traits;

trait Streams
{
    protected static $response;

    public function stream($name, $content, $replace = false)
    {
        static::ensureStreamResponseStarted();

        static::streamContent(['name' => $name, 'content' => $content, 'replace' => $replace]);
    }

    public static function ensureStreamResponseStarted()
    {
        if (static::$response) {
            return;
        }

        $demoCallback = function ($content) {
            while (true) {
                $this->stream('messagestreamtest', 'hi');
                sleep(1);
            }
        };

        static::$response = response()->stream($demoCallback, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'X-Accel-Buffering' => 'no',
            //            'X-Livewire-Stream' => true,
        ]);

        static::$response->sendHeaders();
    }

    public static function streamContent($body)
    {
        $name = $body['name'];
        $content = $body['content'];

        echo "event: $name\n";
        echo "data: $content\n\n";

        //        echo json_encode(['stream' => true, 'body' => $body, 'endStream' => true]);

        if (ob_get_level() > 0) {
            ob_flush();
        }

        flush();
    }
}
