package com.openagents.app

import android.Manifest
import android.content.pm.PackageManager
import android.util.Size
import android.view.ViewGroup
import androidx.activity.ComponentActivity
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import com.google.zxing.BinaryBitmap
import com.google.zxing.NotFoundException
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.multi.qrcode.QRCodeMultiReader
import java.util.concurrent.Executors

/**
 * Scans one QR code with the back camera. Frames stay on the device; the
 * result is bounded text that Rust validates, never a URL to open. Adapted
 * from Coder's Android scanner.
 */
class QRScanner(private val activity: ComponentActivity) {
    private val executor = Executors.newSingleThreadExecutor()
    private var provider: ProcessCameraProvider? = null
    @Volatile private var generation = 0L
    private var preview: Preview? = null
    private var analysis: ImageAnalysis? = null
    private var disposed = false

    fun start(container: ViewGroup, maxBytes: Int, result: (Result<String>) -> Unit) {
        stop()
        val current = generation
        if (ContextCompat.checkSelfPermission(activity, Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) {
            result(Result.failure(IllegalStateException("Camera access is off. Turn it on in Settings, or type the code instead."))); return
        }
        val nativePreview = PreviewView(activity).apply { contentDescription = "Camera preview for scanning an invitation" }
        container.addView(nativePreview, ViewGroup.LayoutParams(-1, activity.dp(220)))
        val future = ProcessCameraProvider.getInstance(activity)
        future.addListener({
            if (disposed || generation != current) return@addListener
            try {
                val camera = future.get()
                provider = camera
                val display = Preview.Builder().build().also { it.surfaceProvider = nativePreview.surfaceProvider }
                @Suppress("DEPRECATION")
                val frames = ImageAnalysis.Builder().setTargetResolution(Size(1280, 720))
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST).build()
                preview = display; analysis = frames
                frames.setAnalyzer(executor) { proxy ->
                    try {
                        if (generation == current && proxy.width in 1..2048 && proxy.height in 1..2048) {
                            val plane = proxy.planes.first()
                            val buffer = plane.buffer
                            val luminance = ByteArray(proxy.width * proxy.height)
                            val start = buffer.position()
                            for (y in 0 until proxy.height) for (x in 0 until proxy.width) {
                                luminance[y * proxy.width + x] = buffer.get(start + y * plane.rowStride + x * plane.pixelStride)
                            }
                            val source = PlanarYUVLuminanceSource(luminance, proxy.width, proxy.height,
                                0, 0, proxy.width, proxy.height, false)
                            val found = try { decode(BinaryBitmap(HybridBinarizer(source)), maxBytes) } catch (_: NotFoundException) { null }
                            if (found != null) activity.runOnUiThread {
                                if (generation == current) { stop(); result(found) }
                            }
                        }
                    } catch (_: Exception) {
                        activity.runOnUiThread {
                            if (generation == current) { stop(); result(Result.failure(IllegalStateException("The camera couldn't read this code. Type it instead."))) }
                        }
                    } finally { proxy.close() }
                }
                camera.bindToLifecycle(activity, CameraSelector.DEFAULT_BACK_CAMERA, display, frames)
            } catch (_: Exception) {
                stop(); result(Result.failure(IllegalStateException("The camera couldn't start. Type the code instead.")))
            }
        }, ContextCompat.getMainExecutor(activity))
    }

    fun stop() {
        generation += 1
        analysis?.clearAnalyzer()
        val owned = listOfNotNull(preview, analysis).toTypedArray()
        if (owned.isNotEmpty()) provider?.unbind(*owned)
        preview = null; analysis = null
    }

    fun dispose() { stop(); disposed = true; executor.shutdown() }

    private fun decode(bitmap: BinaryBitmap, maxBytes: Int): Result<String> {
        val codes = QRCodeMultiReader().decodeMultiple(bitmap)
        return runCatching {
            val text = codes.singleOrNull()?.text ?: throw IllegalArgumentException("Show one QR code at a time.")
            require(text.toByteArray().size <= maxBytes) { "That QR code is too long." }
            text
        }
    }
}
