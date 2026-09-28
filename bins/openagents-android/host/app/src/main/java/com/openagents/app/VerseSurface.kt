package com.openagents.app

import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.os.SystemClock
import android.view.Choreographer
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.SurfaceHolder
import android.view.SurfaceView
import org.json.JSONArray
import org.json.JSONObject

/**
 * The Verse tab's world: Verse's bare plaza grid with Coder's player
 * controls, drawn by Rust into this SurfaceView. Android owns the surface,
 * the display clock, touches, and the rotation sensor; Rust owns the world,
 * the player, the camera, the movement stick, and every frame. Adapted from
 * Coder's Android `VerseSurface`. Every Verse JNI call stays on the main thread.
 */
class VerseSurface(context: Context, private val changed: (JSONObject?, String?) -> Unit) :
    SurfaceView(context), SurfaceHolder.Callback, Choreographer.FrameCallback, SensorEventListener {
    private var handle = 0L
    private var attached = false
    private var disposed = false
    private var visible = false
    private var resumed = false
    private var running = false
    private var lastFrame = 0L
    private var lastPublish = 0L
    private var lastSample = 0L
    private var sensorStarted = 0L
    private var latestSensor: Pair<Long, DoubleArray>? = null
    private var sensorRunning = false
    private val pointers = mutableSetOf<Int>()
    private var hudInsets = floatArrayOf(0f, 0f, 0f, 0f)
    private val pinchAdmission = PinchAdmission()
    private val pinch = ScaleGestureDetector(context, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
        override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
            if (!running || !pinchAdmission.allowed) return false
            cancelPointers(retainContacts = true)
            return true
        }
        override fun onScale(detector: ScaleGestureDetector): Boolean {
            val scale = detector.scaleFactor
            if (running && scale.isFinite() && scale > 0f) send(json("action" to "pinch_zoom", "scale" to scale), false)
            return true
        }
    }).apply { isQuickScaleEnabled = false; isStylusScaleEnabled = false }
    private val sensors = context.getSystemService(SensorManager::class.java)
    private val sensor = sensors?.getDefaultSensor(Sensor.TYPE_GAME_ROTATION_VECTOR)
        ?: sensors?.getDefaultSensor(Sensor.TYPE_ROTATION_VECTOR)
    val motionAvailable get() = sensor != null
    var snapshot: JSONObject? = null; private set
    var motionError: String? = null; private set
    var nativeError: String? = null; private set

    init {
        tag = "verse-surface"
        contentDescription = "Verse world. Drag to look around, and push the stick at the bottom left to walk. Double-tap to jump. Pinch with two fingers to zoom."
        holder.addCallback(this)
        isFocusable = true
    }

    override fun surfaceCreated(holder: SurfaceHolder) = Unit
    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        if (disposed || width <= 0 || height <= 0) return
        when {
            handle == 0L -> create()
            !attached -> attach()
            else -> send(json("action" to "resize", "width" to width, "height" to height,
                "scale" to resources.displayMetrics.density))
        }
    }
    override fun surfaceDestroyed(holder: SurfaceHolder) = detachSurface()

    private fun surfaceConfig() = json("width" to width, "height" to height,
        "scale" to resources.displayMetrics.density).toString()

    private fun create() {
        if (disposed || !holder.surface.isValid || width <= 0 || height <= 0) return
        try {
            // Avatar presence signs with its own world key, never the device key.
            // Without one, the world stays offline.
            val config = JSONObject(surfaceConfig())
            runCatching { DeviceKey.loadOrCreate(context, DeviceKey.Purpose.WORLD) }
                .onSuccess { config.put("world_secret_hex", it) }
            handle = OpenAgentsNative.verseCreate(holder.surface, config.toString())
            check(handle != 0L) { "The world renderer couldn't start on this device." }
            attached = true
            sendHudInsets()
            send(json("action" to "snapshot"))
            updateActivity()
        } catch (failure: Exception) {
            handle = 0
            nativeError = failure.message ?: "The world couldn't start."
            changed(null, nativeError)
        }
    }

    private fun attach() {
        if (disposed || handle == 0L || attached || !holder.surface.isValid || width <= 0 || height <= 0) return
        try {
            OpenAgentsNative.verseAttach(handle, holder.surface, surfaceConfig())
            attached = true
            sendHudInsets()
            send(json("action" to "snapshot"))
            updateActivity()
        } catch (failure: Exception) {
            nativeError = failure.message ?: "The world renderer couldn't reattach. Try again."
            changed(null, nativeError)
        }
    }

    fun retry() {
        if (disposed) return
        detachSurface()
        if (handle == 0L) create() else attach()
    }

    /** Space the system bars and the tab bar cover, in density-independent pixels. */
    fun setHudInsets(top: Float, right: Float, bottom: Float, left: Float) {
        val next = floatArrayOf(top, right, bottom, left)
        if (next.any { !it.isFinite() || it < 0f } || next.contentEquals(hudInsets)) return
        hudInsets = next
        cancelTouches()
        sendHudInsets()
    }
    private fun sendHudInsets() = send(json("action" to "hud_insets", "top" to hudInsets[0],
        "right" to hudInsets[1], "bottom" to hudInsets[2], "left" to hudInsets[3]))

    /** The Verse tab is showing. The world runs only while shown and resumed. */
    fun setShown(value: Boolean) { visible = value; updateActivity() }
    fun setResumed(value: Boolean) { resumed = value; updateActivity() }

    private fun updateActivity() {
        val next = visible && resumed && !disposed && attached && handle != 0L && holder.surface.isValid
        if (next == running) return
        running = next
        if (!next) { stopSensors(); cancelTouches(); Choreographer.getInstance().removeFrameCallback(this) }
        lastFrame = 0
        send(json("action" to "active", "active" to next))
        if (next) Choreographer.getInstance().postFrameCallback(this)
    }

    override fun doFrame(frameTimeNanos: Long) {
        if (!running) return
        // About 30 frames per second, as in Coder's Android host.
        if (lastFrame == 0L || frameTimeNanos - lastFrame >= 32_000_000L) {
            lastFrame = frameTimeNanos
            pollMotion()
            send(json("action" to "frame", "timestamp" to frameTimeNanos / 1e9), false)
        }
        if (running) Choreographer.getInstance().postFrameCallback(this)
    }

    fun send(request: JSONObject, publish: Boolean = true): JSONObject? {
        if (handle == 0L) return null
        return try {
            val encoded = request.toString()
            require(encoded.toByteArray().size <= 4096) { "The world request is too large." }
            val result = packet(OpenAgentsNative.verseCall(handle, encoded), "coder.verse.v1")
            require(result.getJSONArray("position").length() == 3 &&
                result.getString("camera_mode") in listOf("touch", "motion")) { "Invalid world view." }
            snapshot = result
            if (BuildConfig.DEBUG) contentDescription = "Verse world. ${json(
                "frames" to result.optLong("frames_presented"),
                "position" to result.getJSONArray("position"),
                "camera_yaw" to result.optDouble("camera_yaw"),
                "camera_distance" to result.optDouble("camera_distance"),
                "camera_mode" to result.optString("camera_mode"))}"
            nativeError = null
            syncSensors(result.optBoolean("motion_needed"))
            val now = SystemClock.elapsedRealtimeNanos()
            if (publish || result.textOrNull("error") != null || now - lastPublish >= 150_000_000) {
                lastPublish = now
                changed(result, null)
            }
            result
        } catch (failure: Exception) {
            nativeError = failure.message ?: "The world couldn't update."
            changed(null, nativeError); null
        }
    }

    fun toggleMotion() {
        if (snapshot?.optString("camera_mode") == "motion") {
            motionError = null; send(json("action" to "camera_mode", "mode" to "touch"))
        } else if (motionAvailable) {
            motionError = null; send(json("action" to "camera_mode", "mode" to "motion"))
        } else { motionError = "Motion look isn't available. Touch look still works."; changed(snapshot, null) }
    }

    fun recenter() = send(json("action" to "recenter_camera"))

    private fun syncSensors(needed: Boolean) {
        if (!needed || !running) { stopSensors(); return }
        if (sensorRunning) return
        sensorStarted = SystemClock.elapsedRealtimeNanos(); lastSample = 0; latestSensor = null
        sensorRunning = sensor != null && sensors?.registerListener(this, sensor, 16_667) == true
        if (!sensorRunning) motionFailed("Motion updates couldn't start. Touch look is on.")
    }
    private fun stopSensors() {
        if (sensorRunning) sensors?.unregisterListener(this)
        sensorRunning = false; latestSensor = null; lastSample = 0
    }
    private fun motionFailed(message: String) {
        stopSensors(); motionError = message
        send(json("action" to "camera_mode", "mode" to "touch"))
    }
    private fun pollMotion() {
        if (!sensorRunning) return
        val now = SystemClock.elapsedRealtimeNanos()
        val receivedAt = System.nanoTime()
        val sample = latestSensor
        if (sample != null && sample.first >= sensorStarted && sample.first > lastSample &&
            sample.first <= now + 5_000_000 && now - sample.first <= 250_000_000) {
            lastSample = sample.first
            send(json("action" to "device_motion", "quaternion" to JSONArray(sample.second.toList()),
                "timestamp" to (sample.first + receivedAt - now) / 1e9,
                "received_at" to receivedAt / 1e9), false)
        } else if (now - (if (lastSample == 0L) sensorStarted else lastSample) >
            if (lastSample == 0L) 2_000_000_000 else 1_000_000_000) {
            motionFailed("Motion updates stopped. Touch look is on.")
        }
    }
    override fun onSensorChanged(event: SensorEvent) {
        if (!sensorRunning || event.values.size < 3) return
        val quaternion = FloatArray(4)
        SensorManager.getQuaternionFromVector(quaternion, event.values)
        // Both native hosts supply device-to-world quaternions (x, y, z, w).
        val sample = doubleArrayOf(quaternion[1].toDouble(), quaternion[2].toDouble(),
            quaternion[3].toDouble(), quaternion[0].toDouble())
        if (sample.all { it.isFinite() }) latestSensor = event.timestamp to sample
    }
    override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) = Unit

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (!running) { cancelTouches(); return false }
        val index = event.actionIndex
        val density = resources.displayMetrics.density
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                if (event.actionMasked == MotionEvent.ACTION_DOWN) { cancelPointers(); pinchAdmission.reset() }
                val id = event.getPointerId(index)
                if (pointers.size < 8) {
                    pointers.add(id)
                    pointer(event, index, "down")
                    pinchAdmission.down(id, event.getX(index) / density, event.getY(index) / density, event.eventTime)
                }
            }
            MotionEvent.ACTION_MOVE -> for (i in 0 until event.pointerCount) {
                pinchAdmission.move(event.getPointerId(i), event.getX(i) / density, event.getY(i) / density)
            }
        }
        if (pinchAdmission.reserved) cancelPointers(retainContacts = true)
        pinch.onTouchEvent(event)
        when (event.actionMasked) {
            MotionEvent.ACTION_MOVE -> for (i in 0 until event.pointerCount) {
                val id = event.getPointerId(i)
                if (id in pointers && !pinchAdmission.reserved) pointer(event, i, "move")
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
                val id = event.getPointerId(index)
                if (pointers.remove(id) && !pinchAdmission.reserved) pointer(event, index, "up")
                pinchAdmission.up(id)
                performClick()
            }
            MotionEvent.ACTION_CANCEL -> { cancelPointers(); pinchAdmission.reset() }
        }
        return true
    }
    override fun performClick(): Boolean { super.performClick(); return true }

    private fun pointer(event: MotionEvent, index: Int, phase: String) {
        val density = resources.displayMetrics.density
        send(json("action" to "pointer", "id" to event.getPointerId(index), "phase" to phase,
            "x" to event.getX(index) / density, "y" to event.getY(index) / density), phase != "move")
    }

    /** Sends a synthetic touch path, for the debug `verse_script` launch extra. */
    fun scriptPointer(id: Int, phase: String, x: Float, y: Float) {
        if (running) send(json("action" to "pointer", "id" to id, "phase" to phase, "x" to x, "y" to y), phase != "move")
    }

    private fun cancelPointers(retainContacts: Boolean = false) {
        for (id in pointers.toList()) {
            send(json("action" to "pointer", "id" to id, "phase" to "cancel", "x" to 0, "y" to 0), false)
            if (!retainContacts) pointers.remove(id)
        }
    }
    private fun cancelTouches() {
        cancelPointers()
        pinchAdmission.reset()
        val now = SystemClock.uptimeMillis()
        val cancel = MotionEvent.obtain(now, now, MotionEvent.ACTION_CANCEL, 0f, 0f, 0)
        pinch.onTouchEvent(cancel)
        cancel.recycle()
    }

    private fun detachSurface() {
        running = false
        Choreographer.getInstance().removeFrameCallback(this)
        stopSensors(); cancelTouches()
        if (handle != 0L && attached) {
            send(json("action" to "active", "active" to false), false)
            try {
                // A Surface can disappear while the Activity keeps the world.
                // Drop GPU resources before Android releases the window.
                OpenAgentsNative.verseDetach(handle)
                attached = false
            } catch (failure: Exception) {
                nativeError = failure.message ?: "The world renderer couldn't suspend."
                changed(null, nativeError)
            }
        }
        lastFrame = 0
    }

    /** Releases the world; only the Activity's disposal calls this. */
    fun release() {
        if (disposed) return
        disposed = true
        detachSurface()
        if (handle != 0L) {
            try { OpenAgentsNative.verseDestroy(handle) } catch (_: Exception) {}
            handle = 0; attached = false
        }
        holder.removeCallback(this)
    }
}
