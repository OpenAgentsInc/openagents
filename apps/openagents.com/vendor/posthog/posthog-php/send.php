<?php

require_once __DIR__ . '/vendor/autoload.php';

/**
 * require client
 */

use PostHog\PostHog;

/**
 * Args
 */

$args = parse($argv);

/**
 * Make sure both are set
 */

if (!isset($args["apiKey"])) die("--apiKey must be given");
if (!isset($args["file"])) die("--file must be given");

$file = $args["file"];
if ($file[0] != '/') $file = __DIR__ . "/" . $file;

/**
 * Rename the file so we don't write the same calls
 * multiple times
 */

$dir = dirname($file);
$old = $file;
$file = $dir . '/posthog-' . rand() . '.log';

if(!file_exists($old)) {
  print("file: $old does not exist");
  exit(0);
}

if (!rename($old, $file)) {
  print("error renaming from $old to $file\n");
  exit(1);
}

/**
 * File contents.
 */

$contents = file_get_contents($file);
$lines = explode("\n", $contents);

/**
 * Initialize the client.
 */

PostHog::init($args["apiKey"], array(
  "debug" => true,
  "error_handler" => function($code, $msg){
    print("$code: $msg\n");
    exit(1);
  }
));

/**
 * Payloads
 */

$total = 0;
$successful = 0;
foreach ($lines as $line) {
  if (!trim($line)) continue;
  $payload = json_decode($line, true);
  $type = $payload["type"];
  $ret = call_user_func_array(array("PostHog\\PostHog", "raw"), array($payload));
  if ($ret) $successful++;
  $total++;
  if ($total % 100 === 0) PostHog::flush();
}

PostHog::flush();
unlink($file);

/**
 * Sent
 */

print("sent $successful from $total requests successfully");
exit(0);

/**
 * Parse arguments
 */

function parse($argv){
  $ret = array();

  for ($i = 0; $i < count($argv); ++$i) {
    $arg = $argv[$i];
    if ('--' != substr($arg, 0, 2)) continue;
    $ret[substr($arg, 2, strlen($arg))] = trim($argv[++$i]);
  }

  return $ret;
}
