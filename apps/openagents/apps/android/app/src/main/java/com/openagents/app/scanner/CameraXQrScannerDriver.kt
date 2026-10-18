package com.openagents.app.scanner

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Handler
import android.os.Looper
import android.util.Size
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.NotFoundException
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import java.nio.ByteBuffer
import java.util.concurrent.Executor
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong

/** Captures QR frames with CameraX and sends decoded raw text to the Rust classifier boundary. */
class CameraXQrScannerDriver(
    context: Context,
    private val lifecycleOwner: LifecycleOwner,
    private val previewView: PreviewView,
    private val presentation: AndroidScannerPresentationBoundary,
    private val session: Long,
    private val onFailure: (Exception) -> Unit,
) : AutoCloseable {
    private val appContext = context.applicationContext
    private val mainExecutor: Executor = Executor { command -> Handler(Looper.getMainLooper()).post(command) }
    private val analyzerExecutor = Executors.newSingleThreadExecutor()
    private val scanner = ZxingQrDecoder()
    private val preview = Preview.Builder().build()
    private val analysis = ImageAnalysis.Builder()
        .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
        .setTargetResolution(Size(MAX_FRAME_WIDTH, MAX_FRAME_HEIGHT))
        .build()
    private val generation = AtomicLong(1)
    private val closed = AtomicBoolean(false)
    private val started = AtomicBoolean(false)
    private val captureStopped = AtomicBoolean(false)
    private val decoderClosed = AtomicBoolean(false)
    private var cameraProvider: ProcessCameraProvider? = null
    private val decodedText = ScannerDecodedTextDispatcher(presentation, session, ::stopCapture)

    private val lifecycleObserver = LifecycleEventObserver { _, event ->
        when (event) {
            Lifecycle.Event.ON_STOP -> cancel()
            Lifecycle.Event.ON_DESTROY -> close()
            else -> Unit
        }
    }

    init {
        require(session > 0)
        lifecycleOwner.lifecycle.addObserver(lifecycleObserver)
        preview.setSurfaceProvider(previewView.surfaceProvider)
    }

    /** Start capture after the Activity has received camera permission. */
    fun start(): Boolean {
        if (closed.get() || presentation.state != AndroidScannerState.SCANNING || !started.compareAndSet(false, true)) return false
        if (appContext.checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) {
            presentation.permissionDenied(session)
            stopCapture()
            return false
        }

        val startGeneration = generation.get()
        return try {
            val providerFuture = ProcessCameraProvider.getInstance(appContext)
            providerFuture.addListener({
                if (!isCurrent(startGeneration)) return@addListener
                try {
                    cameraProvider = providerFuture.get()
                    bindCamera(startGeneration)
                } catch (error: Exception) {
                    fail(startGeneration, error)
                }
            }, mainExecutor)
            true
        } catch (error: Exception) {
            fail(startGeneration, error)
            false
        }
    }

    /** Stop camera resources after cancellation; callers can create a new driver after retry. */
    fun stopCapture() {
        if (!captureStopped.compareAndSet(false, true)) return
        decodedText.close()
        analysis.clearAnalyzer()
        analyzerExecutor.shutdown()
        closeDecoder()
        mainExecutor.execute {
            cameraProvider?.unbind(preview, analysis)
            cameraProvider = null
        }
    }

    /** Cancel capture while leaving the presentation boundary eligible for an explicit retry. */
    fun cancel() {
        presentation.cancel(session)
        stopCapture()
    }

    /** Close capture, decoder, analyzer, and all callbacks from this driver. */
    override fun close() {
        if (!closed.compareAndSet(false, true)) return
        generation.incrementAndGet()
        stopCapture()
        decodedText.close()
        analyzerExecutor.shutdownNow()
        lifecycleOwner.lifecycle.removeObserver(lifecycleObserver)
        presentation.close()
    }

    private fun bindCamera(startGeneration: Long) {
        if (!isCurrent(startGeneration) || lifecycleOwner.lifecycle.currentState == Lifecycle.State.DESTROYED) return
        analysis.setAnalyzer(analyzerExecutor) { imageProxy ->
            if (!isCurrent(startGeneration)) {
                imageProxy.close()
                return@setAnalyzer
            }
            if (appContext.checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) {
                presentation.permissionDenied(session)
                stopCapture()
                imageProxy.close()
                return@setAnalyzer
            }
            try {
                val yPlane = imageProxy.planes.firstOrNull()
                val rawText = yPlane?.let {
                    scanner.decode(
                        it.buffer,
                        imageProxy.width,
                        imageProxy.height,
                        it.rowStride,
                        it.pixelStride,
                        imageProxy.imageInfo.rotationDegrees,
                    )
                }
                if (rawText != null && isCurrent(startGeneration)) {
                    mainExecutor.execute {
                        if (!isCurrent(startGeneration)) return@execute
                        try {
                            decodedText.onDecoded(rawText)
                        } catch (error: Exception) {
                            stopCapture()
                            onFailure(error)
                        }
                    }
                }
            } catch (error: Exception) {
                mainExecutor.execute { fail(startGeneration, error) }
            } finally {
                imageProxy.close()
            }
        }
        cameraProvider?.bindToLifecycle(
            lifecycleOwner,
            CameraSelector.DEFAULT_BACK_CAMERA,
            preview,
            analysis,
        )
    }

    private fun fail(startGeneration: Long, error: Exception) {
        if (!isCurrent(startGeneration)) return
        if (presentation.captureFailed(session)) {
            stopCapture()
            onFailure(error)
        }
    }

    private fun isCurrent(expectedGeneration: Long): Boolean =
        !closed.get() && !captureStopped.get() && generation.get() == expectedGeneration

    private fun closeDecoder() {
        if (decoderClosed.compareAndSet(false, true)) scanner.close()
    }

    private companion object {
        const val MAX_FRAME_WIDTH = 1280
        const val MAX_FRAME_HEIGHT = 720
    }
}

/** Decodes bounded CameraX Y-plane frames with the ZXing core library. */
internal class ZxingQrDecoder : AutoCloseable {
    private val reader = MultiFormatReader().apply {
        setHints(
            mapOf(
                DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE),
            ),
        )
    }
    private var closed = false

    @Synchronized
    fun decode(
        yPlane: ByteBuffer,
        width: Int,
        height: Int,
        rowStride: Int,
        pixelStride: Int,
        rotationDegrees: Int,
    ): String? {
        val frame = compactAndRotateYPlane(yPlane, width, height, rowStride, pixelStride, rotationDegrees) ?: return null
        if (closed) return null
        val source = PlanarYUVLuminanceSource(
            frame.bytes,
            frame.width,
            frame.height,
            0,
            0,
            frame.width,
            frame.height,
            false,
        )
        return try {
            reader.decodeWithState(BinaryBitmap(HybridBinarizer(source))).text
        } catch (_: NotFoundException) {
            null
        } finally {
            reader.reset()
        }
    }

    @Synchronized
    override fun close() {
        if (closed) return
        closed = true
        reader.reset()
    }

    internal data class GrayFrame(val bytes: ByteArray, val width: Int, val height: Int)

    internal fun compactAndRotateYPlane(
        yPlane: ByteBuffer,
        width: Int,
        height: Int,
        rowStride: Int,
        pixelStride: Int,
        rotationDegrees: Int,
    ): GrayFrame? {
        if (width !in 1..MAX_FRAME_DIMENSION || height !in 1..MAX_FRAME_DIMENSION) return null
        if (width * height > MAX_FRAME_PIXELS) return null
        if (rowStride <= 0 || pixelStride <= 0 || rotationDegrees !in setOf(0, 90, 180, 270)) return null
        if (rowStride < (width - 1) * pixelStride + 1) return null
        val lastOffset = (height - 1L) * rowStride + (width - 1L) * pixelStride
        val input = yPlane.duplicate()
        if (lastOffset >= input.remaining()) return null

        val compact = ByteArray(width * height)
        val base = input.position()
        for (y in 0 until height) {
            val inputRow = base + y * rowStride
            val outputRow = y * width
            for (x in 0 until width) {
                compact[outputRow + x] = input.get(inputRow + x * pixelStride)
            }
        }

        if (rotationDegrees == 0) return GrayFrame(compact, width, height)
        val rotatedWidth = if (rotationDegrees == 180) width else height
        val rotatedHeight = if (rotationDegrees == 180) height else width
        val rotated = ByteArray(rotatedWidth * rotatedHeight)
        for (y in 0 until rotatedHeight) {
            for (x in 0 until rotatedWidth) {
                val sourceX: Int
                val sourceY: Int
                when (rotationDegrees) {
                    90 -> {
                        sourceX = y
                        sourceY = height - 1 - x
                    }
                    180 -> {
                        sourceX = width - 1 - x
                        sourceY = height - 1 - y
                    }
                    else -> {
                        sourceX = width - 1 - y
                        sourceY = x
                    }
                }
                rotated[y * rotatedWidth + x] = compact[sourceY * width + sourceX]
            }
        }
        return GrayFrame(rotated, rotatedWidth, rotatedHeight)
    }

    private companion object {
        const val MAX_FRAME_DIMENSION = 1280
        const val MAX_FRAME_PIXELS = 1280 * 720
    }
}

/** Serializes decoded values and sends at most one value from a live camera session to Rust. */
internal class ScannerDecodedTextDispatcher(
    private val presentation: AndroidScannerPresentationBoundary,
    private val session: Long,
    private val onSubmitted: () -> Unit,
) {
    private val lock = Any()
    private var active = true

    fun onDecoded(rawText: String?): AndroidScanResult? {
        if (rawText == null) return null
        val result = synchronized(lock) {
            if (!active) return@synchronized AndroidScanResult.STALE_SESSION
            presentation.onScanText(session, rawText).also {
                if (it == AndroidScanResult.SUBMITTED_TO_RUST) active = false
            }
        }
        if (result == AndroidScanResult.SUBMITTED_TO_RUST) onSubmitted()
        return result
    }

    fun close() = synchronized(lock) { active = false }
}
