package com.openagents.coder

import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.SystemClock
import android.text.Selection
import android.text.Spannable
import android.view.MotionEvent
import android.view.InputDevice
import android.view.View
import android.view.ViewGroup
import android.widget.CompoundButton
import android.widget.EditText
import android.widget.TextView
import androidx.lifecycle.Lifecycle
import androidx.test.core.app.ActivityScenario
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TestName
import org.junit.runner.RunWith
import java.io.File
import java.security.SecureRandom
import java.security.MessageDigest
import java.util.UUID
import java.util.concurrent.Executors
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import kotlin.math.abs

/** Native acceptance uses isolated synthetic state and never starts a model or recipe. */
@RunWith(AndroidJUnit4::class)
class MobileAcceptanceTest {
    @get:Rule val testName = TestName()
    private var scenario: ActivityScenario<MainActivity>? = null
    private val instrumentation = InstrumentationRegistry.getInstrumentation()

    @After fun finish() {
        if (scenario != null) capture(testName.methodName)
        scenario?.close()
        scenario = null
    }

    private fun capture(name: String) {
        if (scenario != null) {
            // A continuously rendered world need not reach main-loop idle.
            // Waiting for idle can exhaust the synthetic Gym snapshot's TTL.
            SystemClock.sleep(200)
            val directory = File(instrumentation.targetContext.getExternalFilesDir(null), "acceptance")
            directory.mkdirs()
            instrumentation.uiAutomation.takeScreenshot()?.let { bitmap ->
                File(directory, "$name.png").outputStream().use {
                    bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)
                }
                bitmap.recycle()
            }
        }
    }

    @Test fun worldRendersFullBleedAndTouchMovesTheSharedPlayer() {
        launch()
        waitFor { frames() >= 3 }
        onMain { activity ->
            val surface = find(activity.window.decorView, "verse-surface") ?: error("Missing surface")
            val location = IntArray(2)
            surface.getLocationOnScreen(location)
            assertEquals("Canvas starts at the left screen edge", 0, location[0])
            assertEquals("Canvas paints behind the system clock", 0, location[1])
            assertEquals(activity.window.decorView.width, surface.width)
            assertEquals(activity.window.decorView.height, surface.height)
        }
        val before = position()
        walk()
        waitFor { distance(position(), before) > 0.1 }
        assertFalse("Renderer error", exists("verse-error", nonempty = true))
        val retained = position()
        val presented = frames()
        onMain { find(it.window.decorView, "verse-surface")!!.visibility = View.GONE }
        SystemClock.sleep(300)
        onMain { find(it.window.decorView, "verse-surface")!!.visibility = View.VISIBLE }
        waitFor { frames() > presented }
        assertTrue("Replacing the native surface retains the player", distance(retained, position()) < 0.05)
    }

    @Test fun cleanWorldUsesDoubleTapJumpAndTwoFingerPinch() {
        launch()
        waitFor { frames() >= 3 }
        for (tag in listOf("verse-status", "verse-jump", "verse-sprint", "verse-zoom-in", "verse-zoom-out",
                "verse-frames", "verse-position", "gym-interest", "verse-camera", "verse-motion-needed",
                "verse-controls-hint")) assertFalse("Removed visible control: $tag", exists(tag))
        assertFalse(hasText("Coder"))
        assertFalse(hasText("Verse offline world"))
        assertTrue("Recenter is available in touch mode", exists("verse-motion-recenter"))
        onMain { activity ->
            for (tag in listOf("verse-camera-mode", "verse-motion-recenter")) {
                val control = find(activity.window.decorView, tag)!!
                assertTrue("Camera control is icon-only", control is android.widget.ImageButton)
                assertTrue("Camera control retains its accessibility name", !control.contentDescription.isNullOrEmpty())
            }
        }
        val initial = position()
        tapSurface(0.82f, 0.32f)
        tapSurface(0.82f, 0.32f)
        waitFor { position().getOrElse(1) { initial[1] } > initial[1] + 0.03 }
        val zoomBefore = worldSnapshot().getDouble("camera_distance")
        val yawBefore = cameraYaw()
        pinchSurface(0.40f, 0.74f)
        waitFor { worldSnapshot().getDouble("camera_distance") < zoomBefore - 0.05 }
        val zoomed = worldSnapshot().getDouble("camera_distance")
        pinchSurface(0.74f, 0.40f)
        waitFor { worldSnapshot().getDouble("camera_distance") > zoomed + 0.05 }
        val afterPinch = position()
        val yawAfterPinch = cameraYaw()
        assertEquals("Two fingers reserve zoom before recognition", yawBefore, yawAfterPinch, 0.001)
        assertTrue("Two fingers do not move the player", distance(initial, afterPinch) < 0.05)
        val pinchFrame = frames()
        waitFor { frames() > pinchFrame + 3 }
        assertEquals("A pinch leaves no held look input", yawAfterPinch, cameraYaw(), 0.001)
        assertTrue("A pinch leaves no held movement", distance(afterPinch, position()) < 0.05)
        assertFalse(exists("verse-error", nonempty = true))
    }

    @Test fun readerPagesStayPinnedAndExactRecordsRemainAccessible() {
        launch()
        computer()
        click("chat-0")
        waitFor { exists("details") }
        assertFalse("Source metadata is collapsed", exists("history-progress"))
        assertFalse("Page metadata is collapsed", exists("page-position"))
        assertFalse("Device details stay in Settings", exists("reader-public-key"))
        click("details")
        waitFor { text("page-position").contains("Page 2 of 2") }
        click("earlier")
        waitFor { text("page-position").contains("Page 1 of 2") }
        assertFalse(checked("timeline-follow"))
        val selected = AtomicReference<TextView>()
        onMain { activity ->
            val view = all(activity.window.decorView).filterIsInstance<TextView>().first {
                (it.tag as? String)?.endsWith("-text") == true && it.text.length > 8
            }
            selected.set(view)
            Selection.setSelection(view.text as Spannable, 1, 6)
        }
        click("reader-refresh")
        waitFor { !exists("reader-status", nonempty = true) }
        assertTrue(text("page-position").contains("Page 1 of 2"))
        onMain { activity ->
            val same = find(activity.window.decorView, selected.get().tag.toString()) as TextView
            assertSame("Refresh retains selectable native text", selected.get(), same)
            assertEquals(1, same.selectionStart)
            assertEquals(6, same.selectionEnd)
        }
        click("later")
        waitFor { text("page-position").contains("Page 2 of 2") }
        assertFalse(checked("timeline-follow"))
        click("follow")
        waitFor { checked("timeline-follow") }
        val raw = AtomicReference<String>()
        onMain { activity ->
            raw.set(all(activity.window.decorView).firstOrNull {
                (it.tag as? String)?.endsWith("-raw") == true
            }?.tag as? String)
        }
        assertNotNull("Exact-source control", raw.get())
        click(raw.get())
        waitFor { hasText("Show readable text") }
        assertFalse("Reader must stay read-only", hasText("Send"))
        capture("reader-exact-record")
        click(raw.get())
        capture("reader-transcript")
        click("back")
        waitFor { exists("chat-0") }
    }

    @Test fun computersScreensShowStatusesAccessAndPastedInvitation() {
        launch()
        computer()
        waitFor { exists("chat-0") }
        openComputers()
        waitFor { exists("first-run-title") }
        capture("computers-first-run")
        click("first-run-continue")
        // First run hands back to the existing chats flow.
        waitFor { exists("chat-0") }
        openComputers()
        waitFor { exists("computers-title") }
        for ((key, words) in listOf("host-0-status" to "Online", "host-1-status" to "Connecting",
                "host-2-status" to "Offline", "host-3-status" to "Out of date",
                "host-4-status" to "Not enrolled", "host-5-status" to "Revoked",
                "host-6-status" to "switched off")) {
            assertTrue("$key: ${text(key)}", text(key).contains(words))
        }
        capture("computers-statuses")
        click("host-0-access")
        waitFor { exists("access-title") }
        assertFalse("Revoking this device is disabled", exists("device-0-revoke", enabled = true))
        assertTrue(text("device-0-revoke-reason").contains("device you're using"))
        capture("computers-access")
        click("tab-computers")
        click("host-0-switch")
        waitFor { text("host-0-status").contains("switched off") }
        click("tab-add")
        waitFor { exists("invite-paste") }
        assertFalse("The Phone projection omits SSH", exists("ssh-connect"))
        assertFalse("The Phone projection omits the SSH section", exists("ssh-title"))
        click("invite-paste")
        waitFor { exists("computers-input") }
        onMain { (find(it.window.decorView, "computers-input") as EditText).setText("coder-host:emulator") }
        click("computers-submit")
        waitFor { text("notice").contains("Added New computer.") }
        capture("computers-added")
        click("tab-activity")
        waitFor { text("activity-0-subject").contains("Build server") }
        capture("computers-activity")
    }

    @Test fun ownerKeyInputIsMaskedAndClearedOnCancel() {
        launch()
        computer()
        waitFor { exists("chat-0") }
        openComputers()
        waitFor { exists("first-run-title") }
        click("first-run-continue")
        waitFor { exists("chat-0") }
        openComputers()
        click("directory-owner-key")
        waitFor { exists("computers-input") }
        onMain { activity ->
            val field = find(activity.window.decorView, "computers-input") as EditText
            assertEquals(android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD,
                field.inputType and android.text.InputType.TYPE_MASK_VARIATION)
            assertTrue(field.transformationMethod is android.text.method.PasswordTransformationMethod)
            assertFalse("Key input is not saved in view state", field.isSaveEnabled)
            assertEquals(View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS, field.importantForAutofill)
            // This deliberately invalid marker is never submitted or persisted.
            field.setText("not-a-real-owner-key")
        }
        click("computers-cancel")
        waitFor { !exists("computers-input") }
        click("directory-owner-key")
        waitFor { exists("computers-input") }
        onMain { activity ->
            assertEquals("", (find(activity.window.decorView, "computers-input") as EditText).text.toString())
        }
        click("computers-cancel")
    }

    /**
     * Enter owner key over the synthetic fixture. Its owner key is a public
     * test value, not a credential. The field must be a masked password input.
     */
    @Test fun ownerKeyIsMaskedAndAcceptedOnlyWhenAGrantNamesIt() {
        val owner = "0e".repeat(32)
        val other = "0f".repeat(32)
        launch()
        computer()
        waitFor { exists("chat-0") }
        openComputers()
        waitFor { exists("first-run-title") }
        click("first-run-continue")
        waitFor { exists("chat-0") }
        openComputers()
        waitFor { exists("computers-title") }
        click("directory-owner-key")
        waitFor { exists("computers-input") }
        onMain {
            val field = find(it.window.decorView, "computers-input") as EditText
            val variation = field.inputType and android.text.InputType.TYPE_MASK_VARIATION
            assertEquals("A secret is a password input", android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD, variation)
            assertTrue("A secret is masked", field.transformationMethod is android.text.method.PasswordTransformationMethod)
            assertFalse("A secret is never saved with the view state", field.isSaveEnabled)
            field.setText(other)
        }
        capture("owner-key-masked")
        click("computers-submit")
        waitFor { text("notice").contains("isn't the owner key") }
        assertFalse("A refused key is never echoed", hasText(other))
        waitFor { exists("computers-input") }
        onMain { (find(it.window.decorView, "computers-input") as EditText).setText(owner) }
        click("computers-submit")
        waitFor { text("notice").contains("now holds your owner key") }
        waitFor { text("directory-status").contains("Your directory is empty") }
        assertFalse(exists("directory-owner-key"))
        assertFalse(exists("computers-input"))
        assertFalse("The accepted key is never echoed", hasText(owner))
        capture("owner-key-accepted")
    }

    /**
     * A build with google-services.json and loopback push settings (see the
     * README) reports its wake status: Rust's after it receives the FCM token,
     * or the native failure. Default builds skip it.
     */
    @Test fun pushConfiguredBuildReportsItsWakeStatus() {
        org.junit.Assume.assumeTrue("This build is not configured for push", PushSettings.configured)
        val intent = Intent(ApplicationProvider.getApplicationContext(), MainActivity::class.java)
            .putExtra("synthetic", true).putExtra("loopback_test", true)
        instrumentation.uiAutomation.grantRuntimePermission("com.openagents.coder", "android.permission.POST_NOTIFICATIONS")
        scenario = ActivityScenario.launch(intent)
        waitFor { frames() > 0 }
        computer()
        openSettings()
        click("reader-details")
        waitFor { exists("push-status") }
        waitFor(60_000) { text("push-status").let { it.startsWith("Wakes unavailable:") || it.startsWith("Couldn't register for wakes") } }
        capture("push-status")
    }

    /**
     * The Computers screens against a real host on the build computer. Start the
     * fixture first (`serve_a_host_for_a_device_run` in crates/coder-mobile),
     * forward its relay and host ports with `adb reverse`, and pass its
     * invitation as the `coderLiveInvitation` instrumentation argument.
     * Without it, the test is skipped.
     */
    @Test fun computersLiveEnrollInviteActivityAndRevocation() {
        val invitation = InstrumentationRegistry.getArguments().getString("coderLiveInvitation")
        org.junit.Assume.assumeTrue("No live host invitation", invitation?.startsWith("coder-host:") == true)
        // Start from this test's own empty synthetic Computers record.
        File(DeviceStorage(instrumentation.targetContext, true).cacheDirectory(), "computers").deleteRecursively()
        val intent = Intent(ApplicationProvider.getApplicationContext(), MainActivity::class.java)
            .putExtra("synthetic", true).putExtra("loopback_test", true)
        scenario = ActivityScenario.launch(intent)
        waitFor { frames() > 0 }
        computer()
        openComputers()
        waitFor { exists("first-run-title") }
        click("invite-paste")
        waitFor { exists("computers-input") }
        onMain { (find(it.window.decorView, "computers-input") as EditText).setText(invitation) }
        click("computers-submit")
        waitFor(60_000) { text("notice").contains("Added Computer") }
        click("first-run-continue")
        waitFor { exists("computer-settings", enabled = true) }
        openComputers()
        waitFor(60_000) { text("host-0-status").contains("Online") }
        capture("computers-live-online")
        click("host-0-access")
        waitFor(60_000) { text("device-0-label").contains("last seen") }
        assertFalse(text("device-0-label").contains("unknown"))
        capture("computers-live-last-seen")
        click("share-right-terminal")
        click("share-right-review")
        click("share-create")
        waitFor(30_000) { exists("computers-qr") }
        assertTrue(text("share-code-detail").contains("View sessions and tasks, Run and steer tasks."))
        capture("computers-live-invitation-qr")
        click("share-done")
        click("tab-activity")
        waitFor(90_000) { text("activity-0-headline").contains("Task queued") }
        capture("computers-live-activity")
        click("tab-computers")
        waitFor(150_000) { text("host-0-status").contains("Revoked") }
        assertTrue(text("host-0-status").contains("removed this device's access"))
        capture("computers-live-revoked")
    }

    @Test fun backgroundAndSurfaceRecreationKeepReaderIdentity() {
        launch()
        computer()
        val key = publicKey()
        assertTrue(key.matches(Regex("[0-9a-f]{64}")))
        click("chat-0")
        waitFor { exists("back") }
        click("computer-close")
        val previous = frames()
        val retained = position()
        scenario!!.moveToState(Lifecycle.State.CREATED)
        SystemClock.sleep(300)
        scenario!!.moveToState(Lifecycle.State.RESUMED)
        waitFor { frames() > previous }
        assertTrue("Backgrounding retains the player", distance(retained, position()) < 0.05)
        assertFalse(exists("verse-error", nonempty = true))
        scenario!!.recreate()
        waitFor { frames() > 0 }
        computer()
        assertEquals(key, publicKey())
        assertTrue("Surface presented before background", previous > 0)
    }

    @Test fun invalidPairingIsVisibleAndPasteRemainsAvailable() {
        launch()
        computer()
        openSettings()
        click("computer-pair")
        waitFor { exists("computer-paste") }
        assertEquals("openagents pair", text("computer-command"))
        assertTrue(exists("computer-scan"))
        click("computer-paste")
        waitFor { exists("computer-code") }
        onMain { activity ->
            (find(activity.window.decorView, "computer-code") as EditText)
                .setText("not-a-valid-invitation")
        }
        click("computer-connect")
        waitFor { exists("reader-error", nonempty = true) }
        assertTrue(exists("computer-paste"))
        assertTrue(exists("computer-close"))
    }

    @Test fun worldRelayIsRememberedAcrossPanelAndActivityReopen() {
        DeviceStorage(instrumentation.targetContext, true).saveWorldRelay(null)
        launch()
        computer()
        openSettings()
        click("world-connection")
        assertEquals("Offline", text("world-connection-status"))
        onMain { activity ->
            (find(activity.window.decorView, "world-relay") as EditText).setText("wss://relay.example.test")
        }
        click("world-join")
        waitFor { text("world-connection-status") == "Preview" }
        click("computer-close")
        computer()
        openSettings()
        if (!exists("world-relay")) click("world-connection")
        assertEquals("wss://relay.example.test", text("world-relay"))
        assertEquals("Preview", text("world-connection-status"))
        scenario!!.recreate()
        waitFor { frames() > 0 }
        computer()
        openSettings()
        click("world-connection")
        assertEquals("wss://relay.example.test", text("world-relay"))
        assertEquals("Preview", text("world-connection-status"))
        click("world-leave")
        waitFor { text("world-connection-status") == "Offline" }
        assertNull(DeviceStorage(instrumentation.targetContext, true).worldRelay())
    }

    @Test fun motionModeRecenterAndPauseUseTheSharedController() {
        launch(motion = true)
        waitFor { frames() > 2 }
        click("verse-camera-mode")
        waitFor { worldSnapshot().optBoolean("motion_needed") }
        waitFor { frames() > 5 }
        val yaw = cameraYaw()
        onMain { activity ->
            val surface = find(activity.window.decorView, "verse-surface")!!
            assertTrue(surface.performAccessibilityAction(R.id.verse_inject_motion_action, null))
        }
        waitFor { abs(cameraYaw() - yaw) > 0.05 }
        assertTrue(exists("verse-motion-recenter"))
        val centered = cameraYaw()
        val centeredPosition = position()
        val centeredDistance = worldSnapshot().getDouble("camera_distance")
        click("verse-motion-recenter")
        val recenterFrame = frames()
        waitFor { frames() > recenterFrame + 3 }
        assertEquals("Recenter faces the player heading", centered, cameraYaw(), 0.02)
        assertEquals("Recenter restores the default pitch", 0.28, worldSnapshot().getDouble("camera_pitch"), 0.02)
        assertEquals("Recenter preserves zoom", centeredDistance, worldSnapshot().getDouble("camera_distance"), 0.001)
        assertTrue("Recenter preserves player position", distance(centeredPosition, position()) < 0.05)
        val before = position()
        holdForward()
        waitFor { distance(position(), before) > 0.1 }
        val retained = position()
        val retainedYaw = cameraYaw()
        val presented = frames()
        scenario!!.moveToState(Lifecycle.State.CREATED)
        SystemClock.sleep(300)
        scenario!!.moveToState(Lifecycle.State.RESUMED)
        waitFor { frames() > presented }
        assertTrue("Backgrounding retains motion camera mode", exists("verse-motion-recenter"))
        assertTrue(distance(retained, position()) < 0.05)
        assertEquals(retainedYaw, cameraYaw(), 0.02)
        assertFalse(exists("verse-error", nonempty = true))
        click("verse-camera-mode")
    }

    @Test fun gymHasSyntheticBoardsWithoutLaunchingWork() {
        launch(gym = true)
        waitFor { frames() > 2 }
        assertFalse(worldSnapshot().optBoolean("gym_active"))
        val run = "gym-run-" + "1".repeat(64)
        assertFalse(exists(run))
        for (attempt in 0..12) {
            if (exists("gym-interact", enabled = true)) break
            walk()
        }
        waitFor { worldSnapshot().optBoolean("gym_active") && exists("gym-interact", enabled = true) }
        click("gym-interact")
        waitFor { exists(run) }
        click(run)
        waitFor { exists("gym-run-title") }
        assertTrue(hasText("Cost unavailable · 125 s"))
        click("gym-metric-0-values")
        assertTrue(hasText("Step 24: 8.0 checks"))
        capture("gym-chart")
        click("gym-all-runs")
        click("gym-recipe-preview-recipe")
        waitFor { exists("gym-confirm-launch") }
        assertTrue(hasText("No dollar limit is enforced"))
        assertFalse(exists("gym-launch-status"))
        // This test reviews the recipe without submitting any work.
        capture("gym-recipe")
        click("gym-all-runs")
        click("gym-connection")
        waitFor { exists("gym-code") }
        onMain { (find(it.window.decorView, "gym-code") as EditText).setText("invalid-gym-invitation") }
        click("gym-connect")
        waitFor { exists("gym-error", nonempty = true) }
        assertEquals("invalid-gym-invitation", text("gym-code"))
        capture("gym-connection-error")
        click("gym-back")
        click("gym-close")
        waitFor { !exists("gym-close") }
        for (attempt in 0..15) {
            if (!worldSnapshot().optBoolean("gym_active")) break
            gesture(0.12f, 0.32f, 0.12f, 0.50f, 1200)
        }
        waitFor { !worldSnapshot().optBoolean("gym_active") }
        assertFalse(exists(run))
        assertFalse(exists("verse-error", nonempty = true))
    }

    @Test fun androidQrDecoderMatchesTheRetainedRustInvitation() {
        val bitmap = instrumentation.context.assets.open("synthetic-qr.png").use { BitmapFactory.decodeStream(it) }
        val result = AtomicReference<Result<String>>()
        val done = CountDownLatch(1)
        instrumentation.runOnMainSync {
            QRScanner.decodeBitmap(bitmap) { result.set(it); done.countDown() }
        }
        assertTrue("Bundled decoder completed", done.await(30, TimeUnit.SECONDS))
        val bytes = result.get().getOrThrow().toByteArray(Charsets.UTF_8)
        assertEquals(191, bytes.size)
        val digest = MessageDigest.getInstance("SHA-256").digest(bytes)
            .joinToString("") { "%02x".format(it.toInt() and 255) }
        assertEquals("a9e0729c887fb856a06c83f06993d458692b7ebe848af864d29a88e5e31ce037", digest)
        assertThrows(IllegalArgumentException::class.java) { QRScanner.bounded("https://example.invalid") }
        assertThrows(IllegalArgumentException::class.java) { QRScanner.bounded("coder-pair:" + "x".repeat(641)) }
        bitmap.recycle()
    }

    @Test fun jniRejectsMalformedAndStaleReaderHandles() {
        assertThrows(RuntimeException::class.java) { CoderNative.createReader("{}") }
        assertThrows(RuntimeException::class.java) {
            CoderNative.readerCall(Long.MAX_VALUE, "{\"op\":\"snapshot\"}")
        }
        val directory = File(instrumentation.targetContext.noBackupFilesDir, "jni-${UUID.randomUUID()}")
        val secret = ByteArray(32).also { SecureRandom().nextBytes(it) }
            .joinToString("") { "%02x".format(it.toInt() and 255) }
        val handle = CoderNative.createReader(json("cache_dir" to directory.path,
            "secret_hex" to secret, "synthetic" to true).toString())
        val other = Executors.newSingleThreadExecutor()
        try {
            assertTrue(handle > 0)
            assertTrue(CoderNative.readerCall(handle, "{\"op\":\"snapshot\"}")
                .contains("coder.mobile.v1"))
            assertTrue(other.submit<Boolean> {
                try { CoderNative.readerCall(handle, "{\"op\":\"snapshot\"}"); false }
                catch (_: RuntimeException) { true }
            }.get(10, TimeUnit.SECONDS))
            instrumentation.runOnMainSync {
                assertThrows(RuntimeException::class.java) {
                    CoderNative.verseCall(handle, "{\"action\":\"snapshot\"}")
                }
            }
        } finally { other.shutdownNow(); CoderNative.destroyReader(handle); directory.deleteRecursively() }
        assertThrows(RuntimeException::class.java) {
            CoderNative.readerCall(handle, "{\"op\":\"snapshot\"}")
        }
        // A main-thread render handle cannot be used from this worker thread.
        assertThrows(RuntimeException::class.java) {
            CoderNative.verseCall(Long.MAX_VALUE, "{\"action\":\"snapshot\"}")
        }
    }

    @Test fun mapUsesRealSurfaceTouchesAndManualMovementCancelsWalking() {
        launch()
        waitFor { worldSnapshot().optJSONObject("map")?.optBoolean("visible") == true }
        var width = 1f
        var height = 1f
        onMain { activity ->
            val surface = find(activity.window.decorView, "verse-surface") as VerseSurface
            val density = surface.resources.displayMetrics.density
            width = surface.width / density
            height = surface.height / density
            val labels = surface.createAccessibilityNodeInfo().actionList.map { it.label?.toString() }
            assertTrue(labels.contains("Open map"))
            assertTrue(labels.contains("Walk to Gym"))
        }
        val compact = worldSnapshot().getJSONObject("map").getJSONArray("frame")
        assertTrue(compact.getDouble(1) > 0)
        assertTrue(compact.getDouble(0) + compact.getDouble(2) <= width)
        tapSurface(((compact.getDouble(0) + compact.getDouble(2) / 2) / width).toFloat(),
            ((compact.getDouble(1) + 12) / height).toFloat())
        waitFor { worldSnapshot().getJSONObject("map").getBoolean("expanded") }
        val before = position()
        val map = worldSnapshot().getJSONObject("map")
        val plot = map.getJSONArray("plot")
        val center = map.getJSONArray("center")
        val landmarks = map.getJSONArray("landmarks")
        val gym = (0 until landmarks.length()).map { landmarks.getJSONObject(it) }.first { it.getString("id") == "gym" }
        val x = plot.getDouble(0) + ((gym.getDouble("x") - center.getDouble(0)) / map.getDouble("half_extent") + 1) * plot.getDouble(2) / 2
        val y = plot.getDouble(1) + (1 - (gym.getDouble("z") - center.getDouble(1)) / map.getDouble("half_extent")) * plot.getDouble(3) / 2
        tapSurface((x / width).toFloat(), (y / height).toFloat())
        waitFor { worldSnapshot().getJSONObject("map").getString("state") == "Walking" }
        waitFor { distance(before, position()) > 0.3 }
        gesture(0.15f, 0.78f, 0.15f, 0.65f, 200)
        waitFor { worldSnapshot().getJSONObject("map").getString("state") == "Walk stopped" }
        assertTrue(worldSnapshot().textOrNull("error") == null)
    }

    @Test fun projectedCompanionTapReactsWithoutMovingThePlayer() {
        launch()
        waitFor {
            worldSnapshot().optJSONObject("companion")?.let { it.optBoolean("near") && it.optBoolean("visible") } == true
        }
        val before = worldSnapshot()
        val companion = before.getJSONObject("companion")
        onMain { activity ->
            val surface = find(activity.window.decorView, "verse-surface") as VerseSurface
            assertTrue(surface.createAccessibilityNodeInfo().actionList.any { it.label == "Pet companion" })
        }
        tapSurface(companion.getDouble("screen_x").toFloat(), companion.getDouble("screen_y").toFloat())
        waitFor { worldSnapshot().getJSONObject("companion").getLong("pet_count") == companion.getLong("pet_count") + 1 }
        val after = worldSnapshot()
        assertEquals(before.getJSONArray("position").getDouble(0), after.getJSONArray("position").getDouble(0), 0.02)
        assertEquals(before.getJSONArray("position").getDouble(2), after.getJSONArray("position").getDouble(2), 0.02)
        assertEquals(before.getDouble("camera_yaw"), after.getDouble("camera_yaw"), 0.02)
        assertEquals(before.getDouble("camera_pitch"), after.getDouble("camera_pitch"), 0.02)
        assertFalse(after.getBoolean("computer_open"))
        assertNull(after.textOrNull("error"))
        waitFor {
            worldSnapshot().getJSONObject("companion").let { !it.getBoolean("reacting") && it.getDouble("cooldown_seconds") == 0.0 }
        }
        assertEquals(companion.getLong("pet_count") + 1, worldSnapshot().getJSONObject("companion").getLong("pet_count"))
    }

    @Test fun doorMemoriesRefuseIncompatibleItemsAndPersistThroughRecreation() {
        val scope = UUID.randomUUID().toString()
        val storage = DeviceStorage(instrumentation.targetContext, true, scope)
        launch(doorScope = scope)
        approachDoor("spark")
        tapDoor("spark")
        waitFor { door("spark").textOrNull("remembered") == "prism" }
        tapDoorItem("ring")
        tapDoor("spark")
        waitFor { worldSnapshot().getJSONObject("doors").getJSONObject("hud").getString("caption").contains("does not fit") }
        assertEquals("prism", door("spark").getString("remembered"))
        assertNotEquals("Walking", worldSnapshot().getJSONObject("map").getString("state"))
        tapDoorItem("empty")
        tapDoor("spark")
        waitFor { door("spark").getString("state") == "selected" }
        assertEquals("Library", door("spark").getString("destination"))
        approachDoor("halo")
        tapDoorItem("ring")
        tapDoor("halo")
        waitFor { door("halo").textOrNull("remembered") == "ring" }
        val saved = storage.doorPreferences()
        assertNotNull(saved)
        val rendered = frames()
        waitFor { frames() > rendered + 20 }
        assertEquals(saved, storage.doorPreferences())
        scenario!!.recreate()
        waitFor { frames() > 0 }
        assertEquals("ring", worldSnapshot().getJSONObject("doors").getString("held"))
        assertEquals("prism", door("spark").getString("remembered"))
        assertEquals("ring", door("halo").getString("remembered"))
        assertEquals("idle", door("halo").getString("state"))
        assertNotEquals("Walking", worldSnapshot().getJSONObject("map").getString("state"))
        approachDoor("halo")
        tapDoorItem("reset")
        waitFor { door("halo").isNull("remembered") }
        assertEquals("prism", door("spark").getString("remembered"))
        scenario!!.recreate()
        waitFor { frames() > 0 }
        assertTrue(door("halo").isNull("remembered"))
        assertEquals("prism", door("spark").getString("remembered"))
        assertEquals("ring", worldSnapshot().getJSONObject("doors").getString("held"))
    }

    @Test fun invalidDoorPreferencesRemainUnchangedUntilAnExplicitChoice() {
        val scope = UUID.randomUUID().toString()
        val storage = DeviceStorage(instrumentation.targetContext, true, scope)
        val unsupported = "{\"v\":99}"
        storage.saveDoorPreferences(unsupported)
        launch(doorScope = scope)
        waitFor { exists("door-storage-error", nonempty = true) }
        assertFalse(exists("door-save-retry"))
        val rendered = frames()
        waitFor { frames() > rendered + 20 }
        assertEquals(unsupported, storage.doorPreferences())
    }

    private fun door(id: String): JSONObject {
        val doors = worldSnapshot().getJSONObject("doors").getJSONArray("doors")
        return (0 until doors.length()).map { doors.getJSONObject(it) }.first { it.getString("id") == id }
    }
    private fun tapLogical(x: Double, y: Double) {
        var width = 1f; var height = 1f
        onMain { activity ->
            val surface = find(activity.window.decorView, "verse-surface")!!
            val density = surface.resources.displayMetrics.density
            width = surface.width / density; height = surface.height / density
        }
        tapSurface((x / width).toFloat(), (y / height).toFloat())
    }
    private fun approachDoor(id: String) {
        val compact = worldSnapshot().getJSONObject("map").getJSONArray("frame")
        tapLogical(compact.getDouble(0) + compact.getDouble(2) / 2, compact.getDouble(1) + 12)
        waitFor { worldSnapshot().getJSONObject("map").getBoolean("expanded") }
        val map = worldSnapshot().getJSONObject("map")
        val landmarks = map.getJSONArray("landmarks")
        val target = (0 until landmarks.length()).map { landmarks.getJSONObject(it) }.first { it.getString("id") == id }
        val plot = map.getJSONArray("plot"); val center = map.getJSONArray("center")
        tapLogical(plot.getDouble(0) + ((target.getDouble("x") - center.getDouble(0)) / map.getDouble("half_extent") + 1) * plot.getDouble(2) / 2,
            plot.getDouble(1) + (1 - (target.getDouble("z") - center.getDouble(1)) / map.getDouble("half_extent")) * plot.getDouble(3) / 2)
        waitFor { worldSnapshot().getJSONObject("map").getString("state") == "Arrived" }
        var width = 1f
        onMain { activity ->
            val surface = find(activity.window.decorView, "verse-surface")!!
            width = surface.width / surface.resources.displayMetrics.density
        }
        for (attempt in 0 until 16) {
            if (door(id).getBoolean("near") && door(id).getBoolean("visible") && worldSnapshot().getJSONObject("doors").getJSONObject("hud").getBoolean("visible")) break
            val yaw = worldSnapshot().getDouble("camera_yaw")
            val wrapped = kotlin.math.atan2(kotlin.math.sin(yaw), kotlin.math.cos(yaw))
            if (abs(wrapped) < 0.05) break
            val points = (wrapped / 0.004).coerceIn(-60.0, 60.0)
            gesture(0.75f, 0.48f, 0.75f + (points / width).toFloat(), 0.48f, 0)
        }
        waitFor { worldSnapshot().getJSONObject("doors").getJSONObject("hud").let { it.getBoolean("visible") && it.getString("door") == id } }
    }
    private fun tapDoor(id: String) {
        val target = door(id)
        assertTrue(target.getBoolean("near") && target.getBoolean("visible"))
        tapSurface(target.getDouble("screen_x").toFloat(), target.getDouble("screen_y").toFloat())
    }
    private fun tapDoorItem(id: String) {
        val hud = worldSnapshot().getJSONObject("doors").getJSONObject("hud")
        assertTrue(hud.getBoolean("visible"))
        val buttons = hud.getJSONArray("buttons")
        val button = (0 until buttons.length()).map { buttons.getJSONObject(it) }.first { it.getString("id") == id && it.getBoolean("enabled") }
        val frame = button.getJSONArray("frame")
        tapLogical(frame.getDouble(0) + frame.getDouble(2) / 2, frame.getDouble(1) + frame.getDouble(3) / 2)
        if (id != "reset") waitFor { worldSnapshot().getJSONObject("doors").getString("held") == id }
    }

    private fun launch(gym: Boolean = false, motion: Boolean = false, doorScope: String? = null) {
        val intent = Intent(ApplicationProvider.getApplicationContext(), MainActivity::class.java)
            .putExtra("synthetic", true).putExtra("gym_preview", gym).putExtra("motion_preview", motion).putExtra("door_scope", doorScope)
        scenario = ActivityScenario.launch(intent)
        waitFor { frames() > 0 }
    }

    private fun computer() {
        waitFor { frames() > 0 }
        for (attempt in 0..5) {
            if (computerReady()) break
            walk()
        }
        waitFor { computerReady() }
        assertFalse("The computer is drawn in the world, not as a native button", exists("computer-interact"))
        val target = FloatArray(2)
        onMain { activity ->
            val surface = find(activity.window.decorView, "verse-surface") as VerseSurface
            val monitor = surface.snapshot!!.getJSONObject("computer")
            target[0] = monitor.getDouble("screen_x").toFloat()
            target[1] = monitor.getDouble("screen_y").toFloat()
            val actions = surface.createAccessibilityNodeInfo().actionList
            assertTrue("The world retains an accessible computer action", actions.any { it.label == "Use computer" })
        }
        if (testName.methodName == "readerPagesStayPinnedAndExactRecordsRemainAccessible") capture("physical-world-computer")
        val positionBeforeTap = position()
        val yawBeforeTap = cameraYaw()
        tapSurface(target[0], target[1])
        waitFor { exists("chat-0") || exists("back") }
        assertTrue("Tapping the monitor does not move the player", distance(positionBeforeTap, position()) < 0.05)
        assertEquals("Tapping the monitor does not turn the camera", yawBeforeTap, cameraYaw(), 0.001)
        if (testName.methodName == "readerPagesStayPinnedAndExactRecordsRemainAccessible") capture("physical-world-computer-open")
    }

    private fun computerReady(): Boolean {
        var ready = false
        onMain { activity ->
            val computer = (find(activity.window.decorView, "verse-surface") as VerseSurface)
                .snapshot?.optJSONObject("computer")
            ready = computer?.optBoolean("near") == true && computer.optBoolean("visible")
        }
        return ready
    }

    private fun tapSurface(x: Float, y: Float) {
        val bounds = IntArray(4)
        onMain { activity ->
            val surface = find(activity.window.decorView, "verse-surface")!!
            surface.getLocationOnScreen(bounds)
            bounds[2] = surface.width; bounds[3] = surface.height
        }
        val now = SystemClock.uptimeMillis()
        for (action in listOf(MotionEvent.ACTION_DOWN, MotionEvent.ACTION_UP)) {
            val event = MotionEvent.obtain(now, SystemClock.uptimeMillis(), action,
                bounds[0] + bounds[2] * x, bounds[1] + bounds[3] * y, 0)
            instrumentation.sendPointerSync(event)
            event.recycle()
            SystemClock.sleep(60)
        }
    }

    private fun pinchSurface(startSpan: Float, endSpan: Float) {
        val bounds = IntArray(4)
        onMain { activity ->
            val surface = find(activity.window.decorView, "verse-surface")!!
            surface.getLocationOnScreen(bounds)
            bounds[2] = surface.width; bounds[3] = surface.height
        }
        val down = SystemClock.uptimeMillis()
        fun event(action: Int, span: Float, count: Int) {
            val properties = Array(count) { index -> MotionEvent.PointerProperties().apply {
                id = index; toolType = MotionEvent.TOOL_TYPE_FINGER
            } }
            val coordinates = Array(count) { index -> MotionEvent.PointerCoords().apply {
                x = bounds[0] + bounds[2] * (0.5f + if (index == 0) -span / 2 else span / 2)
                y = bounds[1] + bounds[3] * 0.32f
                pressure = 1f; size = 1f
            } }
            val touch = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, count,
                properties, coordinates, 0, 0, 1f, 1f, 0, 0, InputDevice.SOURCE_TOUCHSCREEN, 0)
            instrumentation.sendPointerSync(touch)
            touch.recycle()
        }
        event(MotionEvent.ACTION_DOWN, startSpan, 1)
        event(MotionEvent.ACTION_POINTER_DOWN or (1 shl MotionEvent.ACTION_POINTER_INDEX_SHIFT), startSpan, 2)
        for (step in 1..12) {
            SystemClock.sleep(20)
            event(MotionEvent.ACTION_MOVE, startSpan + (endSpan - startSpan) * step / 12, 2)
        }
        event(MotionEvent.ACTION_POINTER_UP or (1 shl MotionEvent.ACTION_POINTER_INDEX_SHIFT), endSpan, 2)
        // The remaining finger must not become a new movement or look gesture.
        event(MotionEvent.ACTION_MOVE, endSpan + 0.2f, 1)
        SystemClock.sleep(120)
        event(MotionEvent.ACTION_UP, endSpan + 0.2f, 1)
    }

    private fun openSettings() {
        if (!exists("computer-pair")) click("computer-settings")
        // The first click from Computers returns to Chats.
        if (!exists("computer-pair")) click("computer-settings")
        waitFor { exists("computer-pair") }
    }

    private fun openComputers() {
        openSettings()
        click("computers-toggle")
    }

    private fun publicKey(): String {
        openSettings()
        if (!exists("reader-public-key")) click("reader-details")
        waitFor { text("reader-public-key").length == 64 }
        val value = text("reader-public-key")
        click("computer-settings")
        return value
    }

    private fun walk() = gesture(0.12f, 0.50f, 0.12f, 0.32f, 1200)
    private fun holdForward() = gesture(0.12f, 0.50f, 0.12f, 0.50f, 1000)

    private fun gesture(x0: Float, y0: Float, x1: Float, y1: Float, hold: Long) {
        val bounds = IntArray(4)
        onMain { activity ->
            val surface = find(activity.window.decorView, "verse-surface")!!
            surface.getLocationOnScreen(bounds)
            bounds[2] = surface.width; bounds[3] = surface.height
        }
        val down = SystemClock.uptimeMillis()
        fun event(action: Int, x: Float, y: Float) {
            val touch = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action,
                bounds[0] + bounds[2] * x, bounds[1] + bounds[3] * y, 0)
            instrumentation.sendPointerSync(touch)
            touch.recycle()
        }
        event(MotionEvent.ACTION_DOWN, x0, y0)
        for (step in 1..8) {
            SystemClock.sleep(25)
            event(MotionEvent.ACTION_MOVE, x0 + (x1-x0)*step/8, y0 + (y1-y0)*step/8)
        }
        SystemClock.sleep(hold)
        event(MotionEvent.ACTION_UP, x1, y1)
    }

    private fun click(tag: String) {
        waitFor { exists(tag, enabled = true) && !exists("reader-status", nonempty = true) }
        onMain { activity ->
            val view = find(activity.window.decorView, tag)!!
            assertTrue("Click $tag", view.performClick())
        }
    }

    private fun exists(tag: String, enabled: Boolean = false, nonempty: Boolean = false): Boolean {
        var result = false
        onMain { activity ->
            val view = find(activity.window.decorView, tag)
            result = view != null && view.isShown &&
                (!enabled || view.isEnabled) && (!nonempty || (view is TextView && view.text.isNotBlank()))
        }
        return result
    }

    private fun text(tag: String): String {
        var result = ""
        onMain { result = (find(it.window.decorView, tag) as? TextView)?.text?.toString() ?: "" }
        return result
    }

    private fun checked(tag: String): Boolean {
        var result = false
        onMain { result = (find(it.window.decorView, tag) as? CompoundButton)?.isChecked == true }
        return result
    }

    private fun hasText(value: String): Boolean {
        var result = false
        onMain { activity -> result = all(activity.window.decorView).any {
            it.isShown && it is TextView && it.text.toString().contains(value)
        } }
        return result
    }

    private fun cameraYaw(): Double {
        var yaw = 0.0
        onMain { activity ->
            yaw = (find(activity.window.decorView, "verse-surface") as VerseSurface)
                .snapshot?.optDouble("camera_yaw") ?: 0.0
        }
        return yaw
    }

    private fun worldSnapshot(): JSONObject {
        var snapshot = JSONObject()
        onMain { activity ->
            snapshot = (find(activity.window.decorView, "verse-surface") as VerseSurface).snapshot ?: JSONObject()
        }
        return snapshot
    }
    private fun frames() = worldSnapshot().optLong("frames_presented")
    private fun position(): List<Double> {
        val position = worldSnapshot().optJSONArray("position") ?: return emptyList()
        return (0 until position.length()).map { position.getDouble(it) }
    }
    private fun distance(a: List<Double>, b: List<Double>): Double =
        if (a.size == 3 && b.size == 3) abs(a[0]-b[0]) + abs(a[2]-b[2]) else 0.0

    private fun waitFor(timeout: Long = 45_000, predicate: () -> Boolean) {
        val deadline = SystemClock.uptimeMillis() + timeout
        while (SystemClock.uptimeMillis() < deadline) {
            if (predicate()) return
            SystemClock.sleep(100)
        }
        fail("Timed out in ${testName.methodName}; renderer=${text("verse-error")}")
    }

    private fun onMain(block: (MainActivity) -> Unit) { scenario!!.onActivity(block) }
    private fun find(root: View, tag: String): View? = all(root).firstOrNull { it.tag == tag }
    private fun all(root: View): Sequence<View> = sequence {
        yield(root)
        if (root is ViewGroup) for (index in 0 until root.childCount) yieldAll(all(root.getChildAt(index)))
    }
}
