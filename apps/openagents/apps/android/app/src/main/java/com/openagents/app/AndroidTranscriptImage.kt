package com.openagents.app

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.view.View
import android.widget.ImageView
import kotlinx.coroutines.*
import java.util.concurrent.atomic.AtomicReference

/** Decodes Rust-authorized bytes in a fixed widget rectangle; owns all native image memory. */
internal class AndroidTranscriptImage(
    context: Context,
    scope: CoroutineScope,
    load: suspend () -> ByteArray?,
) : AutoCloseable {
    val view = ImageView(context).apply {
        scaleType = ImageView.ScaleType.FIT_CENTER
        isSaveEnabled = false
        // The surrounding C2 host owns the Rust widget's accessibility and hit actions.
        importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
    }
    private var live = true
    private var bitmap: Bitmap? = null
    private val job = scope.launch {
        val bytes = AtomicReference<ByteArray?>(null)
        val decoded = AtomicReference<Bitmap?>(null)
        try {
            withContext(Dispatchers.IO) { bytes.set(load()) }
            val payload = bytes.get() ?: return@launch
            withContext(Dispatchers.Default) {
                if (payload.size > 16 * 1024 * 1024) return@withContext
                val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeByteArray(payload, 0, payload.size, bounds)
                if (bounds.outWidth !in 1..8192 || bounds.outHeight !in 1..8192) return@withContext
                var sample = 1
                // At most 2 MiB decoded pixels per visible ordinary-image widget.
                while (bounds.outWidth.toLong() / sample * (bounds.outHeight.toLong() / sample) > 512 * 1024) sample *= 2
                decoded.set(BitmapFactory.decodeByteArray(payload, 0, payload.size,
                    BitmapFactory.Options().apply { inSampleSize = sample }))
            }
            if (live) {
                bitmap = decoded.getAndSet(null)
                view.setImageBitmap(bitmap)
            }
        } catch (cancelled: CancellationException) { throw cancelled }
        catch (_: Exception) { }
        catch (_: OutOfMemoryError) { }
        finally {
            bytes.getAndSet(null)?.fill(0)
            decoded.getAndSet(null)?.recycle()
        }
    }
    override fun close() {
        if (!live) return
        live = false
        job.cancel()
        view.setImageDrawable(null)
        bitmap?.recycle(); bitmap = null
    }
}
