// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.composer

import android.app.Dialog
import android.content.Context
import android.view.Gravity
import android.view.ViewGroup
import android.widget.Button
import android.widget.LinearLayout
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import com.openagents.app.MainActivity
import java.io.Closeable
import java.io.File
import java.util.UUID

/** CameraX capture to the app's private cache; no gallery or file URI is exposed. */
internal class AndroidComposerCameraCapture(
    private val activity: MainActivity,
    context: Context,
    private val onCaptured: (File) -> Unit,
    private val onFailure: () -> Unit,
) : Closeable {
    private val density = context.resources.displayMetrics.density
    private val cacheDir = context.cacheDir
    private val dialog = Dialog(activity)
    private val previewView = PreviewView(context)
    private var provider: ProcessCameraProvider? = null
    private var capture: ImageCapture? = null
    private var previewUseCase: Preview? = null
    private var output: File? = null
    private var closed = false
    private var delivered = false

    fun start() {
        val content = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            previewView.layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(360),
            )
            addView(previewView)
            addView(LinearLayout(activity).apply {
                gravity = Gravity.END or Gravity.CENTER_VERTICAL
                addView(Button(activity).apply {
                    text = "Cancel"
                    setOnClickListener { close() }
                })
                addView(Button(activity).apply {
                    text = "Take photo"
                    setOnClickListener { takePhoto() }
                })
            }, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        }
        dialog.setContentView(content)
        dialog.setOnDismissListener { close() }
        dialog.show()
        dialog.window?.setLayout(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
        )
        val future = ProcessCameraProvider.getInstance(activity)
        future.addListener({
            if (closed) return@addListener
            try {
                val cameraProvider = future.get()
                provider = cameraProvider
                val preview = Preview.Builder().build().also {
                    it.surfaceProvider = previewView.surfaceProvider
                }
                previewUseCase = preview
                val imageCapture = ImageCapture.Builder()
                    .setCaptureMode(ImageCapture.CAPTURE_MODE_MINIMIZE_LATENCY)
                    .build()
                capture = imageCapture
                cameraProvider.bindToLifecycle(
                    activity,
                    CameraSelector.DEFAULT_BACK_CAMERA,
                    preview,
                    imageCapture,
                )
            } catch (_: Exception) {
                onFailure()
                close()
            }
        }, ContextCompat.getMainExecutor(activity))
    }

    override fun close() {
        if (closed) return
        closed = true
        provider?.let { cameraProvider ->
            cameraProvider.unbind(*listOfNotNull(previewUseCase, capture).toTypedArray())
        }
        provider = null
        capture = null
        previewUseCase = null
        if (dialog.isShowing) dialog.dismiss()
        if (!delivered) output?.delete()
    }

    private fun takePhoto() {
        if (closed) return
        val imageCapture = capture ?: run {
            onFailure()
            return
        }
        val file = File(cacheDir, "composer-${UUID.randomUUID()}.jpg")
        output = file
        val options = ImageCapture.OutputFileOptions.Builder(file).build()
        imageCapture.takePicture(
            options,
            ContextCompat.getMainExecutor(activity),
            object : ImageCapture.OnImageSavedCallback {
                override fun onImageSaved(result: ImageCapture.OutputFileResults) {
                    if (closed) {
                        file.delete()
                        return
                    }
                    delivered = true
                    close()
                    onCaptured(file)
                }

                override fun onError(error: ImageCaptureException) {
                    if (closed) return
                    onFailure()
                    close()
                }
            },
        )
    }

    private fun dp(value: Int): Int = (value * density).toInt()
}
