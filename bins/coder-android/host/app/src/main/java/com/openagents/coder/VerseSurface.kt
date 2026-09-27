package com.openagents.coder

import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.os.Bundle
import android.os.SystemClock
import android.view.Choreographer
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityEvent
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
    var worldStorageError: String? = null
        private set
    var doorStorageError: String? = null
        private set
    var canRetryDoorSave = false
        private set
    private var attemptedDoorRevision: Long? = null
    private var latestDoorDocument: String? = null
    private var doorStorageWrites = 0L
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
    private var computerAccessible = false
    private val pointers = mutableSetOf<Int>()
    private val hudPointers = mutableSetOf<Int>()
    private var hudInsets = floatArrayOf(0f, 0f, 0f, 0f)
    private var accessibilityState = ""
    private val landmarkActions = mutableMapOf<Int, String>()
    private val doorActions = mutableMapOf<Int, Pair<String, JSONObject>>()
    private val pinchAdmission = PinchAdmission()
    private val pinch = ScaleGestureDetector(context, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
        override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
            if (!running || panelOpen() || hudPointers.isNotEmpty() || !pinchAdmission.allowed) return false
            cancelPointers(keepingHud = true)
            return true
        }
        override fun onScale(detector: ScaleGestureDetector): Boolean {
            val scale = detector.scaleFactor
            if (running && !panelOpen() && hudPointers.isEmpty() && scale.isFinite() && scale > 0f) {
                send(json("action" to "pinch_zoom", "scale" to scale), false)
            }
            return true
        }
    }).apply { isQuickScaleEnabled = false; isStylusScaleEnabled = false }
    private val sensors = context.getSystemService(SensorManager::class.java)
    private val sensor = sensors.getDefaultSensor(Sensor.TYPE_GAME_ROTATION_VECTOR)
        ?: sensors.getDefaultSensor(Sensor.TYPE_ROTATION_VECTOR)
    private var previewTurns = 0
    private var previewQuaternion = doubleArrayOf(sqrt(0.5), 0.0, 0.0, sqrt(0.5))
    val motionAvailable get() = motionPreview || sensor != null
    var snapshot: JSONObject? = null; private set
    var motionError: String? = null; private set
    var nativeError: String? = null; private set
    var gymStorageError: String? = null; private set

    init {
        tag = "verse-surface"
        contentDescription = worldDescription
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
                "synthetic" to synthetic, "synthetic_gym" to (synthetic && gymPreview),
                "zone_cache_directory" to java.io.File(context.cacheDir, "VerseZones").absolutePath)
            attemptedDoorRevision = null
            latestDoorDocument = null
            doorStorageWrites = 0
            canRetryDoorSave = false
            try { storage.doorPreferences()?.let { config.put("door_preferences", it) }; doorStorageError = null }
            catch (_: Exception) { doorStorageError = "Saved door choices unavailable. Unlock the device and retry." }
            try {
                storage.worldRelay()?.let { config.put("world_relay", it) }
                config.put("world_offline", storage.worldOffline())
                worldStorageError = null
            } catch (_: Exception) {
                config.put("world_offline", true)
                worldStorageError = "Saved world relay unavailable. Unlock the device and retry."
            }
            storage.gymCode()?.let { config.put("gym_code", it) }
            handle = CoderNative.createVerse(holder.surface, config.toString())
            check(handle != 0L) { packet(CoderNative.verseBlueprint(), "coder.verse.v1").textOrNull("error")
                ?: "The world renderer could not start on this device." }
            attached = true
            sendHudInsets()
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
            sendHudInsets()
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
    fun setHudInsets(top: Float, right: Float, bottom: Float, left: Float) {
        val next = floatArrayOf(top, right, bottom, left)
        if (next.any { !it.isFinite() || it < 0f } || next.contentEquals(hudInsets)) return
        hudInsets = next
        cancelTouches()
        sendHudInsets()
    }
    private fun sendHudInsets() {
        send(json("action" to "hud_insets", "top" to hudInsets[0], "right" to hudInsets[1],
            "bottom" to hudInsets[2], "left" to hudInsets[3]))
    }
    fun setResumed(value: Boolean) { resumed = value; updateActivity() }
    private fun updateActivity() {
        val next = resumed && !disposed && attached && handle != 0L && holder.surface.isValid
        if (next == running) return
        running = next
        if (!next) { stopSensors(); cancelTouches(); Choreographer.getInstance().removeFrameCallback(this) }
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
            retainDoorPreferences(result)
            if (request.optString("action") in listOf("connect", "disconnect") && result.textOrNull("error") == null) {
                try {
                    storage.saveWorldRelay(result.getJSONObject("connection").textOrNull("relay"))
                    worldStorageError = null
                } catch (_: Exception) {
                    worldStorageError = "Could not save the world relay change. Retry before closing Coder."
                }
            }
            if (synthetic) contentDescription = "$worldDescription ${json(
                "frames" to result.optLong("frames_presented"),
                "position" to result.getJSONArray("position"),
                "camera_yaw" to result.optDouble("camera_yaw"),
                "camera_pitch" to result.optDouble("camera_pitch"),
                "camera_distance" to result.optDouble("camera_distance"),
                "gym_active" to result.optBoolean("gym_active"),
                "motion_needed" to result.optBoolean("motion_needed"),
                "map" to result.getJSONObject("map"),
                "companion" to result.getJSONObject("companion"),
                "doors" to result.getJSONObject("doors"),
                "zone" to result.getJSONObject("zone"),
                "door_preferences_revision" to result.getLong("door_preferences_revision"),
                "door_storage_writes" to doorStorageWrites)}"
            val available = computerAvailable()
            val map = result.getJSONObject("map")
            val advertisedDoors = advertisedDoorActions() + advertisedZoneActions()
            val mapState = "${advertisedDoors}:$available:${companionAvailable()}:${map.optBoolean("visible")}:${map.optBoolean("expanded")}:${!map.isNull("destination")}:${map.optJSONArray("landmarks")}"
            if (accessibilityState != mapState) {
                computerAccessible = available
                accessibilityState = mapState
                landmarkActions.clear()
                doorActions.clear()
                for (action in advertisedDoors) doorActions[android.view.View.generateViewId()] = action
                val landmarks = map.optJSONArray("landmarks") ?: JSONArray()
                for (index in 0 until minOf(landmarks.length(), 256)) {
                    landmarkActions[android.view.View.generateViewId()] = landmarks.getJSONObject(index).getString("id")
                }
                sendAccessibilityEvent(AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED)
            }
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

    private fun retainDoorPreferences(packet: JSONObject) {
        val document = packet.getString("door_preferences")
        require(document.toByteArray(Charsets.UTF_8).size <= 2048) { "Invalid door choices." }
        val revision = packet.getLong("door_preferences_revision")
        latestDoorDocument = document
        val previous = attemptedDoorRevision
        attemptedDoorRevision = revision
        // The initial revision must not overwrite unavailable or invalid storage.
        if (previous != null && previous != revision) saveDoorPreferences(document)
    }

    fun retryDoorPreferences() {
        if (!canRetryDoorSave) return
        latestDoorDocument?.let { saveDoorPreferences(it) }
        changed(snapshot, nativeError)
    }

    private fun saveDoorPreferences(document: String) {
        try {
            storage.saveDoorPreferences(document)
            doorStorageWrites += 1
            doorStorageError = null
            canRetryDoorSave = false
        } catch (_: Exception) { doorStorageError = "Door choice not saved."; canRetryDoorSave = true }
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
        previewQuaternion = doubleArrayOf(c, s, s, c)
    }

    private fun syncSensors(needed: Boolean) {
        if (!needed || !running) { stopSensors(); return }
        if (sensorRunning) return
        sensorStarted = SystemClock.elapsedRealtimeNanos(); lastSample = 0; latestSensor = null
        sensorRunning = motionPreview || (sensor != null && sensors.registerListener(this, sensor, 16_667))
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
        val receivedAt = System.nanoTime()
        val sample = if (motionPreview) now to previewQuaternion else latestSensor
        if (sample != null && sample.first >= sensorStarted && sample.first > lastSample &&
            sample.first <= now + 5_000_000 && now - sample.first <= 250_000_000) {
            lastSample = sample.first
            send(json("action" to "device_motion", "quaternion" to JSONArray(sample.second.toList()),
                "timestamp" to (sample.first + receivedAt - now) / 1e9,
                "received_at" to receivedAt / 1e9), false)
        } else if (now - (if (lastSample == 0L) sensorStarted else lastSample) >
            if (lastSample == 0L) 2_000_000_000 else 1_000_000_000) {
            motionFailed("Motion updates stopped. Touch look is active.")
        }
    }
    override fun onSensorChanged(event: SensorEvent) {
        if (!sensorRunning || event.values.size < 3) return
        val quaternion = FloatArray(4)
        SensorManager.getQuaternionFromVector(quaternion, event.values)
        // Both native hosts supply device-to-world quaternions. Rust projects
        // the phone-back direction, removes roll, and smooths the camera.
        val sample = doubleArrayOf(quaternion[1].toDouble(), quaternion[2].toDouble(),
            quaternion[3].toDouble(), quaternion[0].toDouble())
        if (sample.all { it.isFinite() }) latestSensor = event.timestamp to sample
    }
    override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) = Unit

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (!running || panelOpen()) { cancelTouches(); return false }
        val index = event.actionIndex
        val density = resources.displayMetrics.density
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                if (event.actionMasked == MotionEvent.ACTION_DOWN) {
                    cancelPointers()
                    pinchAdmission.reset()
                }
                val id = event.getPointerId(index)
                if (pointers.size < 8) {
                    pointers.add(id)
                    // Rust decides HUD ownership before native pinch admission.
                    pointer(event, index, "down")
                    val captured = listOf(snapshot?.optJSONObject("map")?.optJSONArray("captured_pointers"),
                        snapshot?.optJSONObject("doors")?.optJSONObject("hud")?.optJSONArray("captured_pointers"),
                        snapshot?.optJSONObject("zone")?.optJSONObject("hud")?.optJSONArray("captured_pointers"))
                    if (captured.any { ids -> ids != null && (0 until ids.length()).any { ids.optLong(it) == id.toLong() } }) {
                        hudPointers.add(id)
                    } else {
                        pinchAdmission.down(id, event.getX(index) / density, event.getY(index) / density, event.eventTime)
                    }
                }
            }
            MotionEvent.ACTION_MOVE -> for (i in 0 until event.pointerCount) {
                val id = event.getPointerId(i)
                if (id !in hudPointers) pinchAdmission.move(id, event.getX(i) / density, event.getY(i) / density)
            }
        }
        if (pinchAdmission.reserved) cancelPointers(keepingHud = true, retainContacts = true)
        pinch.onTouchEvent(event)
        when (event.actionMasked) {
            MotionEvent.ACTION_MOVE -> for (i in 0 until event.pointerCount) {
                val id = event.getPointerId(i)
                if (id in pointers && (id in hudPointers || !pinchAdmission.reserved)) pointer(event, i, "move")
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
                val id = event.getPointerId(index)
                if (pointers.remove(id) && (hudPointers.remove(id) || !pinchAdmission.reserved)) pointer(event, index, "up")
                pinchAdmission.up(id)
                performClick()
            }
            MotionEvent.ACTION_CANCEL -> { cancelPointers(); pinchAdmission.reset() }
        }
        return true
    }
    private val worldDescription get() = "Verse world. Drag left to move and right to look around. Double-tap to jump. Pinch with two fingers to zoom. Walk to the computer and tap its screen to open your chats."
    private fun panelOpen() = snapshot?.optBoolean("computer_open") == true || snapshot?.optBoolean("gym_open") == true
    private fun computerAvailable(): Boolean {
        val state = snapshot ?: return false
        val computer = state.optJSONObject("computer") ?: return false
        return running && computer.optBoolean("near") && computer.optBoolean("visible") &&
            !state.optBoolean("computer_open") && !state.optBoolean("gym_open")
    }

    private fun companionAvailable(): Boolean {
        val companion = snapshot?.optJSONObject("companion") ?: return false
        return running && !panelOpen() && companion.optBoolean("near") && companion.optBoolean("visible") &&
            companion.optDouble("cooldown_seconds", 1.0) <= 0
    }

    private fun advertisedDoorActions(): List<Pair<String, JSONObject>> {
        if (!running || panelOpen() || snapshot?.optJSONObject("map")?.optBoolean("expanded") == true) return emptyList()
        val state = snapshot?.optJSONObject("doors") ?: return emptyList()
        val doors = state.optJSONArray("doors") ?: return emptyList()
        val actions = mutableListOf<Pair<String, JSONObject>>()
        for (index in 0 until minOf(doors.length(), 8)) {
            val door = doors.getJSONObject(index)
            val id = door.optString("id")
            if (id in listOf("spark", "halo") && door.optBoolean("near") && door.optBoolean("visible")) {
                actions.add("Use ${door.getString("label")}" to json("action" to "door_tap", "door" to id))
            }
        }
        val hud = state.optJSONObject("hud") ?: return actions
        if (!hud.optBoolean("visible")) return actions
        val buttons = hud.optJSONArray("buttons") ?: return actions
        for (index in 0 until minOf(buttons.length(), 8)) {
            val button = buttons.getJSONObject(index)
            if (!button.optBoolean("enabled")) continue
            val request = button.getJSONObject("action")
            when (request.optString("action")) {
                "door_hold" -> {
                    val item = request.optString("item")
                    if (item in listOf("prism", "ring", "bolt", "empty"))
                        actions.add("Hold ${button.getString("label")}" to json("action" to "door_hold", "item" to item))
                }
                "door_reset" -> {
                    val id = request.optString("door")
                    val target = (0 until doors.length()).map { doors.getJSONObject(it) }.firstOrNull { it.optString("id") == id }
                    if (id in listOf("spark", "halo") && target != null)
                        actions.add("Reset ${target.getString("label")}" to json("action" to "door_reset", "door" to id))
                }
            }
        }
        return actions
    }

    private fun advertisedZoneActions(): List<Pair<String, JSONObject>> {
        if (!running || panelOpen()) return emptyList()
        val hud = snapshot?.optJSONObject("zone")?.optJSONObject("hud") ?: return emptyList()
        if (!hud.optBoolean("visible")) return emptyList()
        val buttons = hud.optJSONArray("buttons") ?: return emptyList()
        val allowed = listOf("enter", "return", "cancel", "retry", "firebolt", "magic_missile", "fireball", "grab", "release", "forces",
            "knob_prev", "knob_next", "decrease", "increase", "reset", "pause", "step")
        return (0 until minOf(buttons.length(), 16)).map { buttons.getJSONObject(it) }
            .filter { it.optBoolean("enabled") && it.optString("action") in allowed }
            .map { it.getString("label") to json("action" to "zone", "intent" to it.getString("action")) }
    }

    override fun onInitializeAccessibilityNodeInfo(info: AccessibilityNodeInfo) {
        super.onInitializeAccessibilityNodeInfo(info)
        for ((action, value) in doorActions) info.addAction(AccessibilityNodeInfo.AccessibilityAction(action, value.first))
        if (synthetic && motionPreview) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction(R.id.verse_inject_motion_action, "Inject motion sample"))
        }
        if (companionAvailable()) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction(R.id.verse_pet_companion_action, "Pet companion"))
        }
        if (computerAvailable()) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction(R.id.verse_use_computer_action, "Use computer"))
        }
        val map = snapshot?.optJSONObject("map")
        if (running && map?.optBoolean("visible") == true) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction(R.id.verse_map_toggle_action,
                if (map.optBoolean("expanded")) "Close map" else "Open map"))
            if (!map.isNull("destination")) info.addAction(AccessibilityNodeInfo.AccessibilityAction(R.id.verse_map_cancel_action, "Cancel walk"))
            for ((action, id) in landmarkActions) {
                landmark(id)?.let { info.addAction(AccessibilityNodeInfo.AccessibilityAction(action, "Walk to ${it.getString("label")}")) }
            }
        }
    }

    private fun landmark(id: String): JSONObject? {
        val landmarks = snapshot?.optJSONObject("map")?.optJSONArray("landmarks") ?: return null
        return (0 until landmarks.length()).map { landmarks.getJSONObject(it) }.firstOrNull { it.optString("id") == id }
    }

    override fun performAccessibilityAction(action: Int, arguments: Bundle?): Boolean {
        doorActions[action]?.let { selected ->
            val current = (advertisedDoorActions() + advertisedZoneActions()).firstOrNull { it.second.toString() == selected.second.toString() } ?: return false
            val result = send(current.second) ?: return false
            return result.textOrNull("error") == null
        }
        if (action == R.id.verse_inject_motion_action && synthetic && motionPreview) {
            injectMotion()
            return true
        }
        if (action == R.id.verse_pet_companion_action) {
            if (!companionAvailable()) return false
            val result = send(json("action" to "pet_companion")) ?: return false
            return result.textOrNull("error") == null
        }
        if (action == R.id.verse_use_computer_action) {
            if (!computerAvailable()) return false
            return send(json("action" to "interact_computer"))?.optBoolean("computer_open") == true
        }
        if (running && snapshot?.optJSONObject("map")?.optBoolean("visible") == true) {
            val request = when (action) {
                R.id.verse_map_toggle_action -> json("action" to "map_toggle")
                R.id.verse_map_cancel_action -> json("action" to "map_cancel")
                else -> landmarkActions[action]?.let { landmark(it) }?.let {
                    json("action" to "map_walk", "x" to it.getDouble("x"), "z" to it.getDouble("z"))
                }
            }
            if (request != null) {
                val result = send(request) ?: return false
                return result.textOrNull("error") == null
            }
        }
        return super.performAccessibilityAction(action, arguments)
    }

    override fun performClick(): Boolean { super.performClick(); return true }
    private fun pointer(event: MotionEvent, index: Int, phase: String) {
        val density = resources.displayMetrics.density
        send(json("action" to "pointer", "id" to event.getPointerId(index), "phase" to phase,
            "x" to event.getX(index) / density, "y" to event.getY(index) / density), phase != "move")
    }
    private fun cancelPointers(keepingHud: Boolean = false, retainContacts: Boolean = false) {
        for (id in pointers.toList()) {
            if (keepingHud && id in hudPointers) continue
            send(json("action" to "pointer", "id" to id, "phase" to "cancel", "x" to 0, "y" to 0), false)
            if (!retainContacts) pointers.remove(id)
        }
        if (!keepingHud) hudPointers.clear()
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
