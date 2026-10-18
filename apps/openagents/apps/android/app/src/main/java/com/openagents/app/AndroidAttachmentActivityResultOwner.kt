// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app

import android.net.Uri
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts

/** Platform result returned to the chat or scanner caller. */
sealed interface AndroidAttachmentResult {
    data class Photo(val uri: Uri) : AndroidAttachmentResult
    data class Document(val uri: Uri) : AndroidAttachmentResult
    data class CameraCapture(val uri: Uri, val saved: Boolean) : AndroidAttachmentResult
    data class CameraPermission(val granted: Boolean) : AndroidAttachmentResult
    data class Cancelled(val source: PickerSource) : AndroidAttachmentResult
}

enum class PickerSource {
    PHOTO,
    DOCUMENT,
}

/** Owns Activity Result launchers for media selection and camera permission. */
class AndroidAttachmentActivityResultOwner(
    activity: ComponentActivity,
    private val onResult: (AndroidAttachmentResult) -> Unit,
) {
    private var closed = false

    private val photoPicker = activity.registerForActivityResult(
        ActivityResultContracts.PickVisualMedia(),
    ) { uri ->
        if (uri == null) dispatch(AndroidAttachmentResult.Cancelled(PickerSource.PHOTO))
        else dispatch(AndroidAttachmentResult.Photo(uri))
    }

    private val documentPicker = activity.registerForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri ->
        if (uri == null) dispatch(AndroidAttachmentResult.Cancelled(PickerSource.DOCUMENT))
        else dispatch(AndroidAttachmentResult.Document(uri))
    }

    private var cameraOutput: Uri? = null
    private val cameraCapture = activity.registerForActivityResult(
        ActivityResultContracts.TakePicture(),
    ) { saved ->
        cameraOutput?.let { dispatch(AndroidAttachmentResult.CameraCapture(it, saved)) }
        cameraOutput = null
    }

    private val cameraPermission = activity.registerForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted -> dispatch(AndroidAttachmentResult.CameraPermission(granted)) }

    fun pickPhoto() {
        if (!closed) photoPicker.launch(
            androidx.activity.result.PickVisualMediaRequest(
                ActivityResultContracts.PickVisualMedia.ImageOnly,
            ),
        )
    }

    fun pickDocument(mimeTypes: Array<String>) {
        require(mimeTypes.isNotEmpty() && mimeTypes.size <= MAX_MIME_TYPES)
        require(mimeTypes.all { it.isNotBlank() && it.length <= MAX_MIME_TYPE_LENGTH })
        if (!closed) documentPicker.launch(mimeTypes.copyOf())
    }

    /** The caller supplies a private output URI before launching the camera app. */
    fun capturePhoto(output: Uri) {
        check(!closed)
        check(cameraOutput == null) { "A camera capture is already active" }
        cameraOutput = output
        cameraCapture.launch(output)
    }

    fun requestCameraPermission() {
        if (!closed) cameraPermission.launch(android.Manifest.permission.CAMERA)
    }

    fun close() {
        closed = true
        cameraOutput = null
    }

    private fun dispatch(result: AndroidAttachmentResult) {
        if (!closed) onResult(result)
    }

    private companion object {
        const val MAX_MIME_TYPES = 16
        const val MAX_MIME_TYPE_LENGTH = 128
    }
}
