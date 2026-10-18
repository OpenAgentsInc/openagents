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
import com.google.mlkit.vision.barcode.Barcode
import com.google.mlkit.vision.barcode.BarcodeScanner
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.barcode.BarcodeScannerOptions
import com.google.mlkit.vision.common.InputImage
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
    private val scanner: BarcodeScanner = BarcodeScanning.getClient(
        BarcodeScannerOptions.Builder().setBarcodeFormats(Barcode.FORMAT_QR_CODE).build(),
    )
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
            val mediaImage = imageProxy.image
            if (mediaImage == null) {
                imageProxy.close()
                return@setAnalyzer
            }
            try {
                scanner.process(InputImage.fromMediaImage(mediaImage, imageProxy.imageInfo.rotationDegrees))
                    .addOnSuccessListener(mainExecutor) { barcodes ->
                        if (!isCurrent(startGeneration)) return@addOnSuccessListener
                        val rawText = barcodes.firstNotNullOfOrNull { it.rawValue }
                        try {
                            decodedText.onDecoded(rawText)
                        } catch (error: Exception) {
                            stopCapture()
                            onFailure(error)
                        }
                    }
                    .addOnFailureListener(mainExecutor) { error -> fail(startGeneration, error) }
                    .addOnCompleteListener(mainExecutor) { imageProxy.close() }
            } catch (error: Exception) {
                imageProxy.close()
                fail(startGeneration, error)
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
