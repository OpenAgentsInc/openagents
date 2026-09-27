package com.openagents.coder

import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.os.SystemClock
import android.view.Choreographer
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.cos
import kotlin.math.sin
import kotlin.math.sqrt

/** The SurfaceView and every Verse JNI call stay on the main thread. */
class VerseSurface(context: Context, private val storage: DeviceStorage,
                   private val synthetic: Boolean, private val gymPreview: Boolean,
                   private val motionPreview: Boolean,
                   private val changed: (JSONObject?, String?) -> Unit) : SurfaceView(context),
    SurfaceHolder.Callback, Choreographer.FrameCallback, SensorEventListener {
    private var handle = 0L
    private var attached = false
    private var disposed = false
    private var resumed = false
    private var running = false
    private var lastFrame = 0L
    private var lastPublish = 0L
    private var lastSample = 0L
    private var sensorStarted = 0L
    private var latestSensor: Pair<Long, DoubleArray>? = null
    private var sensorRunning = false
    private val pointers = mutableSetOf<Int>()
    private val sensors = context.getSystemService(SensorManager::class.java)
    private val sensor = sensors.getDefaultSensor(Sensor.TYPE_GAME_ROTATION_VECTOR)
        ?: sensors.getDefaultSensor(Sensor.TYPE_ROTATION_VECTOR)
    private var previewTurns = 0
    private var previewQuaternion = doubleArrayOf(-sqrt(0.5), 0.0, 0.0, sqrt(0.5))
    val motionAvailable get() = motionPreview || sensor != null
    var snapshot: JSONObject? = null; private set
    var motionError: String? = null; private set
    var nativeError: String? = null; private set
    var gymStorageError: String? = null; private set

    init {
        tag = "verse-surface"
        contentDescription = "Verse world. Drag left to move and right to look around."
        holder.addCallback(this)
        isFocusable = true
    }

    override fun surfaceCreated(holder: SurfaceHolder) = Unit
    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        if (disposed || width <= 0 || height <= 0) return
        when {
            handle == 0L -> create()
            !attached -> attach()
            else -> send(json("action" to "resize", "width" to width,
                "height" to height, "scale" to resources.displayMetrics.density))
        }
    }
    override fun surfaceDestroyed(holder: SurfaceHolder) { detachSurface() }

    private fun create() {
        if (disposed || !holder.surface.isValid || width <= 0 || height <= 0) return
        try {
            val config = json("secret_hex" to storage.identity("verse"), "width" to width,
                "height" to height, "scale" to resources.displayMetrics.density,
                "synthetic" to synthetic, "synthetic_gym" to (synthetic && gymPreview))
            storage.gymCode()?.let { config.put("gym_code", it) }
            handle = CoderNative.createVerse(holder.surface, config.toString())
            check(handle != 0L) { packet(CoderNative.verseBlueprint(), "coder.verse.v1").textOrNull("error")
                ?: "The world renderer could not start on this device." }
            attached = true
            send(json("action" to "snapshot"))
            updateActivity()
        } catch (failure: Exception) {
            nativeError = failure.message ?: "The world could not start."
            changed(null, nativeError)
        }
    }

    private fun attach() {
        if (disposed || handle == 0L || attached || !holder.surface.isValid || width <= 0 || height <= 0) return
        try {
            CoderNative.attachVerse(handle, holder.surface, json("width" to width,
                "height" to height, "scale" to resources.displayMetrics.density).toString())
            attached = true
            send(json("action" to "snapshot"))
            updateActivity()
        } catch (failure: Exception) {
            // Rust retains the suspended Scene after a failed GPU attachment.
            // Retrying must not replace its pose, camera mode, or relay choice.
            nativeError = failure.message ?: "The world renderer could not reattach. Try again."
            changed(null, nativeError)
        }
    }

    fun retry() {
        if (disposed) return
        detachSurface()
        if (handle == 0L) create() else attach()
    }
    fun setResumed(value: Boolean) { resumed = value; updateActivity() }
    private fun updateActivity() {
        val next = resumed && !disposed && attached && handle != 0L && holder.surface.isValid
        if (next == running) return
        running = next
        if (!next) { stopSensors(); cancelPointers(); Choreographer.getInstance().removeFrameCallback(this) }
        lastFrame = 0
        send(json("action" to "active", "active" to next))
        if (next) Choreographer.getInstance().postFrameCallback(this)
    }

    override fun doFrame(frameTimeNanos: Long) {
        if (!running) return
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
            require(encoded.toByteArray().size <= if (request.optString("action") == "gym_configure") 98_304 else 4096)
            val result = packet(CoderNative.verseCall(handle, encoded), "coder.verse.v1")
            require(result.getJSONArray("position").length() == 3 &&
                result.getString("camera_mode") in listOf("touch", "motion")) { "Invalid world view." }
            snapshot = result
            nativeError = null
            syncSensors(result.optBoolean("motion_needed"))
            val now = SystemClock.elapsedRealtimeNanos()
            if (publish || result.textOrNull("error") != null || now - lastPublish >= 150_000_000) {
                lastPublish = now
                changed(result, null)
            }
            result
        } catch (failure: Exception) {
            nativeError = failure.message ?: "The world could not update."
            changed(null, nativeError); null
        }
    }

    fun configureGym(code: String): Boolean {
        if (code.toByteArray().size > 65_536) { changed(null, "The Gym connection exceeds its size limit."); return false }
        val result = send(json("action" to "gym_configure", "code" to code)) ?: return false
        if (result.textOrNull("error") != null || result.optJSONObject("gym_board")?.optBoolean("configured") != true) return false
        try { storage.saveGymCode(code); gymStorageError = null }
        catch (_: Exception) {
            gymStorageError = "The Gym connection works for this session but could not be saved securely."
            changed(null, gymStorageError)
        }
        return true
    }

    fun toggleMotion() {
        if (snapshot?.optString("camera_mode") == "motion") {
            motionError = null; send(json("action" to "camera_mode", "mode" to "touch"))
        } else if (motionAvailable) {
            motionError = null; send(json("action" to "camera_mode", "mode" to "motion"))
        } else { motionError = "Motion look is unavailable. Touch look is still available."; changed(snapshot, null) }
    }

    fun injectMotion() {
        if (!synthetic || !motionPreview) return
        previewTurns += 1
        val angle = previewTurns * 0.25
        val c = sqrt(0.5) * cos(angle / 2)
        val s = sqrt(0.5) * sin(angle / 2)
        previewQuaternion = doubleArrayOf(-c, s, s, c)
    }

    private fun syncSensors(needed: Boolean) {
        if (!needed || !running) { stopSensors(); return }
        if (sensorRunning) return
        sensorStarted = SystemClock.elapsedRealtimeNanos(); lastSample = 0; latestSensor = null
        sensorRunning = motionPreview || (sensor != null && sensors.registerListener(this, sensor, 33_333))
        if (!sensorRunning) motionFailed("Motion updates could not start. Touch look is active.")
    }
    private fun stopSensors() {
        if (sensorRunning && !motionPreview) sensors.unregisterListener(this)
        sensorRunning = false; latestSensor = null; lastSample = 0
    }
    private fun motionFailed(message: String) {
        stopSensors(); motionError = message
        send(json("action" to "camera_mode", "mode" to "touch"))
    }
    private fun pollMotion() {
        if (!sensorRunning) return
        val now = SystemClock.elapsedRealtimeNanos()
        val sample = if (motionPreview) now to previewQuaternion else latestSensor
        if (sample != null && sample.first >= sensorStarted && sample.first > lastSample &&
            sample.first <= now + 250_000_000 && now - sample.first <= 1_000_000_000) {
            lastSample = sample.first
            send(json("action" to "device_motion", "quaternion" to JSONArray(sample.second.toList()),
                "timestamp" to (sample.first + System.nanoTime() - now) / 1e9), false)
        } else if (now - (if (lastSample == 0L) sensorStarted else lastSample) >
            if (lastSample == 0L) 2_000_000_000 else 1_000_000_000) {
            motionFailed("Motion updates stopped. Touch look is active.")
        }
    }
    override fun onSensorChanged(event: SensorEvent) {
        if (!sensorRunning || event.values.size < 3) return
        val quaternion = FloatArray(4)
        SensorManager.getQuaternionFromVector(quaternion, event.values)
        // Android maps device axes into the world. Conjugate to the shared
        // portrait world-to-device contract; Rust removes roll and sets the baseline.
        val sample = doubleArrayOf(-quaternion[1].toDouble(), -quaternion[2].toDouble(),
            -quaternion[3].toDouble(), quaternion[0].toDouble())
        if (sample.all { it.isFinite() }) latestSensor = event.timestamp to sample
    }
    override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) = Unit

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (!running || snapshot?.optBoolean("computer_open") == true || snapshot?.optBoolean("gym_open") == true) return false
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                val index = event.actionIndex
                if (pointers.size < 8) { pointers.add(event.getPointerId(index)); pointer(event, index, "down") }
            }
            MotionEvent.ACTION_MOVE -> for (index in 0 until event.pointerCount)
                if (event.getPointerId(index) in pointers) pointer(event, index, "move")
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
                val index = event.actionIndex
                if (pointers.remove(event.getPointerId(index))) pointer(event, index, "up")
                performClick()
            }
            MotionEvent.ACTION_CANCEL -> cancelPointers()
        }
        return true
    }
    override fun performClick(): Boolean { super.performClick(); return true }
    private fun pointer(event: MotionEvent, index: Int, phase: String) {
        val density = resources.displayMetrics.density
        send(json("action" to "pointer", "id" to event.getPointerId(index), "phase" to phase,
            "x" to event.getX(index) / density, "y" to event.getY(index) / density), phase != "move")
    }
    private fun cancelPointers() {
        for (id in pointers.toList()) send(json("action" to "pointer", "id" to id, "phase" to "cancel", "x" to 0, "y" to 0), false)
        pointers.clear()
    }
    private fun detachSurface() {
        running = false
        Choreographer.getInstance().removeFrameCallback(this)
        stopSensors(); cancelPointers()
        if (handle != 0L) {
            send(json("action" to "active", "active" to false), false)
            try {
                // A Surface can disappear while the Activity still owns the
                // world. Drop GPU resources before Android releases the window.
                CoderNative.detachVerse(handle)
                attached = false
                send(json("action" to "snapshot"))
            } catch (failure: Exception) {
                nativeError = failure.message ?: "The world renderer could not suspend."
                changed(null, nativeError)
            }
        }
        lastFrame = 0
    }

    /** Only disposal of the Activity releases the Rust Scene and its identity. */
    fun release() {
        if (disposed) return
        disposed = true
        detachSurface()
        if (handle != 0L) {
            try { CoderNative.destroyVerse(handle) }
            finally { handle = 0; attached = false }
        }
        holder.removeCallback(this)
    }
}
