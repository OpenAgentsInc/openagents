package com.openagents.coder

import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.SystemClock
import android.text.Selection
import android.text.Spannable
import android.view.MotionEvent
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
            instrumentation.waitForIdleSync()
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
        val frame = frames()
        click("verse-jump")
        waitFor { frames() > frame }
        assertFalse("Renderer error", exists("verse-error", nonempty = true))
        val retained = position()
        val presented = frames()
        onMain { find(it.window.decorView, "verse-surface")!!.visibility = View.GONE }
        SystemClock.sleep(300)
        onMain { find(it.window.decorView, "verse-surface")!!.visibility = View.VISIBLE }
        waitFor { frames() > presented }
        assertTrue("Replacing the native surface retains the player", distance(retained, position()) < 0.05)
    }

    @Test fun readerPagesStayPinnedAndExactRecordsRemainAccessible() {
        launch()
        computer()
        click("chat-0")
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
        waitFor { !text("reader-status").contains("Connecting or updating") }
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
        waitFor { hasText("Show readable record") }
        assertFalse("Reader must stay read-only", hasText("Send"))
        capture("reader-exact-record")
        click(raw.get())
        capture("reader-transcript")
        click("back")
        waitFor { exists("chat-0") }
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
        click("computer-pair")
        waitFor { exists("computer-paste") }
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

    @Test fun motionModeRecenterAndPauseUseTheSharedController() {
        launch(motion = true)
        waitFor { frames() > 2 }
        click("verse-camera-mode")
        waitFor { exists("verse-motion-sample") }
        waitFor { frames() > 5 }
        val yaw = cameraYaw()
        click("verse-motion-sample")
        waitFor { abs(cameraYaw() - yaw) > 0.05 }
        assertTrue(exists("verse-motion-recenter"))
        val centered = cameraYaw()
        click("verse-motion-recenter")
        val recenterFrame = frames()
        waitFor { frames() > recenterFrame + 3 }
        assertEquals("Recenter preserves the view", centered, cameraYaw(), 0.02)
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
        assertEquals("Gym idle", text("gym-interest"))
        val run = "gym-run-" + "1".repeat(64)
        assertFalse(exists(run))
        for (attempt in 0..12) {
            if (exists("gym-interact", enabled = true)) break
            walk()
        }
        waitFor { text("gym-interest") == "Gym listening" && exists("gym-interact", enabled = true) }
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
            if (text("gym-interest") == "Gym idle") break
            gesture(0.12f, 0.32f, 0.12f, 0.50f, 1200)
        }
        waitFor { text("gym-interest") == "Gym idle" }
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

    private fun launch(gym: Boolean = false, motion: Boolean = false) {
        val intent = Intent(ApplicationProvider.getApplicationContext(), MainActivity::class.java)
            .putExtra("synthetic", true).putExtra("gym_preview", gym).putExtra("motion_preview", motion)
        scenario = ActivityScenario.launch(intent)
        waitFor { frames() > 0 }
    }

    private fun computer() {
        waitFor { frames() > 0 }
        for (attempt in 0..5) {
            if (exists("computer-interact", enabled = true)) break
            walk()
        }
        click("computer-interact")
        waitFor { exists("chat-0") || exists("back") }
    }

    private fun publicKey(): String {
        if (!exists("reader-public-key")) click("reader-details")
        waitFor { text("reader-public-key").length == 64 }
        return text("reader-public-key")
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
        waitFor { exists(tag, enabled = true) && !text("reader-status").contains("Connecting or updating") }
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

    private fun frames() = Regex("[0-9]+").find(text("verse-frames"))?.value?.toLongOrNull() ?: 0L
    private fun position(): List<Double> = Regex("-?[0-9]+(?:\\.[0-9]+)?")
        .findAll(text("verse-position")).map { it.value.toDouble() }.toList()
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
