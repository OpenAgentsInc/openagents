package com.openagents.coder

import android.Manifest
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
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
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.multi.qrcode.QRCodeMultiReader
import java.util.concurrent.Executors

/** Camera frames stay on device. A QR result is bounded text, never a URL. */
class QRScanner(private val activity: ComponentActivity) {
    private val executor = Executors.newSingleThreadExecutor()
    private var provider: ProcessCameraProvider? = null
    @Volatile private var generation = 0L
    private var preview: Preview? = null
    private var analysis: ImageAnalysis? = null
    private var disposed = false

    fun start(container: ViewGroup, result: (Result<String>) -> Unit) {
        stop()
        val current = generation
        if (ContextCompat.checkSelfPermission(activity, Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) {
            result(Result.failure(IllegalStateException("Camera access is off. Enable it in Settings, or paste the invitation."))); return
        }
        val nativePreview = PreviewView(activity).apply { contentDescription = "Camera preview for computer invitation" }
        container.addView(nativePreview, ViewGroup.LayoutParams(-1, activity.dp(190)))
        val future = ProcessCameraProvider.getInstance(activity)
        future.addListener({
            if (disposed || generation != current) return@addListener
            try {
                val camera = future.get()
                provider = camera
                val display = Preview.Builder().build().also { it.surfaceProvider = nativePreview.surfaceProvider }
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
                            val found = try { decode(BinaryBitmap(HybridBinarizer(source))) } catch (_: NotFoundException) { null }
                            if (found != null) activity.runOnUiThread {
                                if (generation == current) { stop(); result(found) }
                            }
                        }
                    } catch (_: Exception) {
                        activity.runOnUiThread {
                            if (generation == current) { stop(); result(Result.failure(IllegalStateException("The camera could not read this frame. Paste the invitation instead."))) }
                        }
                    } finally { proxy.close() }
                }
                camera.bindToLifecycle(activity, CameraSelector.DEFAULT_BACK_CAMERA, display, frames)
            } catch (_: Exception) {
                stop(); result(Result.failure(IllegalStateException("The camera could not start. Paste the computer invitation instead.")))
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

    companion object {
        fun bounded(value: String): String {
            require(value.startsWith("coder-pair:") && value.toByteArray().size <= 640) {
                "This QR code is not a Coder invitation. Scan the code displayed by your computer."
            }
            return value
        }
        private fun decode(bitmap: BinaryBitmap): Result<String> {
            val codes = QRCodeMultiReader().decodeMultiple(bitmap)
            return runCatching { bounded(codes.singleOrNull()?.text
                ?: throw IllegalArgumentException("Show one Coder invitation QR code at a time.")) }
        }
        fun decodeBitmap(bitmap: Bitmap, completed: (Result<String>) -> Unit) {
            require(bitmap.width <= 4096 && bitmap.height <= 4096) { "The QR image is too large." }
            Thread({
                val result = runCatching {
                    val pixels = IntArray(bitmap.width * bitmap.height)
                    bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
                    decode(BinaryBitmap(HybridBinarizer(RGBLuminanceSource(bitmap.width, bitmap.height, pixels)))).getOrThrow()
                }
                Handler(Looper.getMainLooper()).post { completed(result) }
            }, "coder-qr-image").start()
        }
    }
}
