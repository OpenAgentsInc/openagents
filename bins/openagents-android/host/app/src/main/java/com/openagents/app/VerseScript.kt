package com.openagents.app

import org.json.JSONObject

/**
 * The debug `verse_script` launch extra, `walk,walk,board`, drives the world
 * through the same pointer path as a finger, so the controls and panels can
 * be checked on an emulator without touching it. Steps follow the iOS
 * `--verse-script`: `walk` pushes the stick forward, `right` pushes it right,
 * `look` drags the view, `turn` holds the look stick right, `walkpinch`
 * holds the stick while two other fingers pinch in, `board` and `results`
 * tap the Gym's boards where the last packet placed them, `r=do:value`
 * sends a results choice, `gym` asks for the Gym board, `everglade` enters
 * Everglade, `station=podium` stands at that station, `interact` sends
 * Everglade's Interact, `s=text` sends text into the open studio panel,
 * `closestudio` closes it, and `wait` does nothing. Each step runs for 90
 * frames after a one-second settle. Debug builds only.
 */
class VerseScript private constructor(private val steps: ArrayDeque<String>) {
    private var frame = 0
    private val pointer = 900

    companion object {
        fun parse(text: String): VerseScript? =
            text.split(',').map { it.trim() }.filter { it.isNotEmpty() }.takeIf { it.isNotEmpty() }
                ?.let { VerseScript(ArrayDeque(it)) }
    }

    /** One frame; `width` and `height` are the surface's size in dp, `top` its top inset. */
    fun step(world: VerseSurface, width: Float, height: Float) {
        frame += 1
        if (frame <= 30) return
        val current = steps.firstOrNull() ?: return
        val t = frame - 31
        // The sticks' centers, as Rust places them: 80 dp in from the side
        // and above the bottom.
        val stickX = 80f; val stickY = height - 80f
        val lookX = width - 80f
        val cx = width / 2; val cy = height * 0.4f
        fun p(id: Int, phase: String, x: Float, y: Float) = world.scriptPointer(id, phase, x, y)
        fun tap(anchor: JSONObject?) {
            anchor ?: return
            val x = anchor.optDouble("screen_x").toFloat() * width; val y = anchor.optDouble("screen_y").toFloat() * height
            if (t == 0) p(pointer, "down", x, y) else if (t == 2) p(pointer, "up", x, y)
        }
        when {
            current == "walk" -> when (t) { 0 -> p(pointer, "down", stickX, stickY); 1 -> p(pointer, "move", stickX, stickY - 56); 80 -> p(pointer, "up", stickX, stickY - 56) }
            current == "right" -> when (t) { 0 -> p(pointer, "down", stickX, stickY); 1 -> p(pointer, "move", stickX + 56, stickY); 40 -> p(pointer, "up", stickX + 56, stickY) }
            current == "left" -> when (t) { 0 -> p(pointer, "down", stickX, stickY); 1 -> p(pointer, "move", stickX - 56, stickY); 40 -> p(pointer, "up", stickX - 56, stickY) }
            current == "turn" -> when (t) { 0 -> p(pointer, "down", lookX, stickY); 1 -> p(pointer, "move", lookX + 40, stickY); 40 -> p(pointer, "up", lookX + 40, stickY) }
            current == "look" -> when (t) {
                0 -> p(pointer, "down", cx, cy)
                in 1..39 -> p(pointer, "move", cx + t * 3, cy + t)
                40 -> p(pointer, "up", cx + 117, cy + 39)
            }
            current == "walkpinch" -> when (t) {
                0 -> p(pointer, "down", stickX, stickY)
                1 -> p(pointer, "move", stickX, stickY - 56)
                in 2..59 -> world.send(json("action" to "pinch_zoom", "scale" to 1.02), false)
                89 -> p(pointer, "up", stickX, stickY - 56)
            }
            current == "board" -> tap(world.snapshot?.objectOrNull("gym"))
            current == "results" -> tap(world.snapshot?.objectOrNull("results"))
            current == "gym" -> if (t == 0) world.send(json("action" to "interact_gym"))
            current == "everglade" -> if (t == 0) world.send(json("action" to "enter_everglade"))
            current == "interact" -> if (t == 0) world.send(json("action" to "zone", "intent" to "interact"))
            current == "closestudio" -> if (t == 0) world.send(json("action" to "close_studio"))
            current.startsWith("station=") -> if (t == 0) world.send(json("action" to "go_station", "station" to current.removePrefix("station=")))
            current.startsWith("s=") -> if (t == 0) world.send(json("action" to "studio_text", "text" to current.removePrefix("s=")))
            current.startsWith("r=") -> if (t == 0) world.results(command(current.removePrefix("r=")))
        }
        if (t >= 89) { steps.removeFirst(); frame = 30 }
    }

    private fun command(text: String): JSONObject {
        val action = text.substringBefore(':')
        val value = text.substringAfter(':', "")
        return when (action) {
            "board", "attempt" -> json("do" to action, "id" to value)
            "filter" -> json("do" to action, "filter" to value.ifEmpty { "all" })
            "tab" -> json("do" to action, "tab" to value.ifEmpty { "jev" })
            "caveats" -> json("do" to action, "open" to (value != "close"))
            "step" -> json("do" to action, "forward" to (value != "back"))
            "seek" -> json("do" to action, "fraction" to (value.toDoubleOrNull() ?: 0.0))
            "play" -> json("do" to action, "playing" to (value != "pause"))
            "page" -> json("do" to action, "page" to (value.toIntOrNull() ?: 0))
            "expand" -> json("do" to action, "index" to (value.toIntOrNull() ?: 0))
            else -> json("do" to action)
        }
    }
}
