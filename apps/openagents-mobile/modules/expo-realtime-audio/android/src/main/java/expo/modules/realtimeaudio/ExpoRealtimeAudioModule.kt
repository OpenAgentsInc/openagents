package expo.modules.realtimeaudio

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioRecord
import android.media.AudioTrack
import android.media.MediaRecorder
import android.util.Base64
import expo.modules.kotlin.exception.CodedException
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

private const val REQUIRED_SAMPLE_RATE = 24_000
private const val MAXIMUM_FRAME_BYTES = 24_000
private const val MAXIMUM_QUEUED_SECONDS = 120
private const val MAXIMUM_QUEUED_BYTES = REQUIRED_SAMPLE_RATE * 2 * MAXIMUM_QUEUED_SECONDS

class ExpoRealtimeAudioModule : Module() {
  private var audioTrack: AudioTrack? = null
  private var executor = Executors.newSingleThreadExecutor()
  private val generation = AtomicInteger(0)
  private val enqueuedFrames = AtomicLong(0)
  private val lifecycleLock = Any()
  private var audioManager: AudioManager? = null
  private var previousAudioMode = AudioManager.MODE_NORMAL
  private var interrupted = false
  private var microphone: AudioRecord? = null
  private var microphoneExecutor = Executors.newSingleThreadExecutor()
  private val microphoneLock = Any()
  private val focusListener = AudioManager.OnAudioFocusChangeListener { focusChange ->
    if (
      focusChange == AudioManager.AUDIOFOCUS_LOSS ||
      focusChange == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT
    ) {
      stop(interruptedBySystem = true)
    }
  }

  override fun definition() = ModuleDefinition {
    Name("ExpoRealtimeAudio")
    Events("onMicrophoneBuffer")

    Function("start") { sampleRate: Int ->
      start(sampleRate)
    }

    Function("enqueue") { pcm16Base64: String ->
      enqueue(pcm16Base64)
    }

    Function("flush") {
      flush()
    }

    Function("stop") {
      stop()
    }

    Function("getStatus") {
      status()
    }

    Function("startMicrophone") { sampleRate: Int ->
      startMicrophone(sampleRate)
    }

    Function("stopMicrophone") {
      stopMicrophone()
    }

    Function("isMicrophoneStarted") {
      synchronized(microphoneLock) {
        microphone?.recordingState == AudioRecord.RECORDSTATE_RECORDING
      }
    }

    OnDestroy {
      stopMicrophone()
      stop()
    }
  }

  private fun startMicrophone(sampleRate: Int) {
    if (sampleRate != REQUIRED_SAMPLE_RATE) {
      throw CodedException("unsupported_sample_rate", "Sarah voice requires 24 kHz PCM.", null)
    }
    synchronized(microphoneLock) {
      if (microphone?.recordingState == AudioRecord.RECORDSTATE_RECORDING) return
      val minimum = AudioRecord.getMinBufferSize(
        sampleRate,
        AudioFormat.CHANNEL_IN_MONO,
        AudioFormat.ENCODING_PCM_16BIT,
      )
      if (minimum <= 0) {
        throw CodedException("audio_buffer_unavailable", "The Android microphone buffer is unavailable.", null)
      }
      val recorder = try {
        AudioRecord.Builder()
          .setAudioSource(MediaRecorder.AudioSource.VOICE_COMMUNICATION)
          .setAudioFormat(
            AudioFormat.Builder()
              .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
              .setSampleRate(sampleRate)
              .setChannelMask(AudioFormat.CHANNEL_IN_MONO)
              .build(),
          )
          .setBufferSizeInBytes(maxOf(minimum, sampleRate / 5))
          .build()
      } catch (error: SecurityException) {
        throw CodedException("microphone_permission_required", "Microphone permission is required.", error)
      }
      if (recorder.state != AudioRecord.STATE_INITIALIZED) {
        recorder.release()
        throw CodedException("microphone_unavailable", "The Android microphone is unavailable.", null)
      }
      if (microphoneExecutor.isShutdown) {
        microphoneExecutor = Executors.newSingleThreadExecutor()
      }
      try {
        recorder.startRecording()
      } catch (error: SecurityException) {
        recorder.release()
        throw CodedException("microphone_permission_required", "Microphone permission is required.", error)
      }
      microphone = recorder
      microphoneExecutor.execute {
        val buffer = ByteArray(sampleRate / 5)
        while (true) {
          if (synchronized(microphoneLock) { microphone !== recorder }) return@execute
          val read = recorder.read(buffer, 0, buffer.size, AudioRecord.READ_BLOCKING)
          if (read <= 0) return@execute
          if (synchronized(microphoneLock) { microphone !== recorder }) return@execute
          sendEvent(
            "onMicrophoneBuffer",
            mapOf(
              "pcm16Base64" to Base64.encodeToString(buffer, 0, read, Base64.NO_WRAP),
              "sampleRate" to REQUIRED_SAMPLE_RATE,
              "channels" to 1,
            ),
          )
        }
      }
    }
  }

  private fun stopMicrophone() {
    synchronized(microphoneLock) {
      val recorder = microphone ?: return
      microphone = null
      microphoneExecutor.shutdownNow()
      try {
        recorder.stop()
      } catch (_: IllegalStateException) {
        // A partially interrupted recorder still needs release.
      }
      recorder.release()
    }
  }

  private fun start(sampleRate: Int) {
    if (sampleRate != REQUIRED_SAMPLE_RATE) {
      throw CodedException("unsupported_sample_rate", "Sarah voice requires 24 kHz PCM.", null)
    }
    synchronized(lifecycleLock) {
      if (audioTrack != null) return
      val context = appContext.reactContext
        ?: throw CodedException("audio_context_unavailable", "The Android audio context is unavailable.", null)
      val manager = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
      val priorMode = manager.mode
      var track: AudioTrack? = null
      try {
        manager.mode = AudioManager.MODE_IN_COMMUNICATION
        @Suppress("DEPRECATION")
        val focusResult = manager.requestAudioFocus(
          focusListener,
          AudioManager.STREAM_VOICE_CALL,
          AudioManager.AUDIOFOCUS_GAIN_TRANSIENT,
        )
        if (focusResult != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
          throw CodedException("audio_focus_unavailable", "Android audio focus is unavailable.", null)
        }
        val minimum = AudioTrack.getMinBufferSize(
          sampleRate,
          AudioFormat.CHANNEL_OUT_MONO,
          AudioFormat.ENCODING_PCM_16BIT,
        )
        if (minimum <= 0) {
          throw CodedException("audio_buffer_unavailable", "The Android PCM output buffer is unavailable.", null)
        }
        track = AudioTrack.Builder()
          .setAudioAttributes(
            AudioAttributes.Builder()
              .setUsage(AudioAttributes.USAGE_VOICE_COMMUNICATION)
              .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
              .build(),
          )
          .setAudioFormat(
            AudioFormat.Builder()
              .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
              .setSampleRate(sampleRate)
              .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
              .build(),
          )
          .setTransferMode(AudioTrack.MODE_STREAM)
          .setBufferSizeInBytes(maxOf(minimum, sampleRate * 2))
          .build()
        if (track.state != AudioTrack.STATE_INITIALIZED) {
          throw CodedException("audio_output_unavailable", "The Android PCM output is unavailable.", null)
        }
        if (executor.isShutdown) {
          executor = Executors.newSingleThreadExecutor()
        }
        generation.incrementAndGet()
        enqueuedFrames.set(0)
        previousAudioMode = priorMode
        audioManager = manager
        audioTrack = track
        interrupted = false
        track.play()
      } catch (error: Throwable) {
        track?.release()
        @Suppress("DEPRECATION")
        manager.abandonAudioFocus(focusListener)
        manager.mode = priorMode
        audioManager = null
        audioTrack = null
        enqueuedFrames.set(0)
        throw error
      }
    }
  }

  private fun enqueue(pcm16Base64: String): Int {
    val bytes = try {
      Base64.decode(pcm16Base64, Base64.DEFAULT)
    } catch (error: IllegalArgumentException) {
      throw CodedException("invalid_pcm_frame", "The PCM playback frame is invalid.", error)
    }
    if (bytes.isEmpty() || bytes.size > MAXIMUM_FRAME_BYTES || bytes.size % 2 != 0) {
      throw CodedException("invalid_pcm_frame", "The PCM playback frame is invalid.", null)
    }
    synchronized(lifecycleLock) {
      val track = audioTrack
        ?: throw CodedException("audio_output_not_started", "PCM output has not started.", null)
      val playbackHead = track.playbackHeadPosition.toLong() and 0xffff_ffffL
      val nextFrames = enqueuedFrames.get() + bytes.size / 2
      if ((nextFrames - playbackHead) * 2 > MAXIMUM_QUEUED_BYTES) {
        throw CodedException("playback_backpressure", "The PCM playback queue is full.", null)
      }
      enqueuedFrames.set(nextFrames)
      val currentGeneration = generation.get()
      executor.execute {
        if (generation.get() != currentGeneration) return@execute
        var offset = 0
        while (offset < bytes.size && generation.get() == currentGeneration) {
          val written = synchronized(lifecycleLock) {
            if (
              generation.get() != currentGeneration ||
              audioTrack !== track
            ) {
              -1
            } else {
              track.write(bytes, offset, bytes.size - offset, AudioTrack.WRITE_BLOCKING)
            }
          }
          if (written <= 0) return@execute
          offset += written
        }
      }
    }
    return bytes.size / 2
  }

  private fun flush() {
    synchronized(lifecycleLock) {
      generation.incrementAndGet()
      val track = audioTrack ?: return
      track.pause()
      track.flush()
      enqueuedFrames.set(0)
      track.play()
    }
  }

  private fun stop(interruptedBySystem: Boolean = false) {
    synchronized(lifecycleLock) {
      generation.incrementAndGet()
      executor.shutdownNow()
      audioTrack?.let { track ->
        try {
          track.pause()
          track.flush()
          track.stop()
        } catch (_: IllegalStateException) {
          // A partially initialized output still needs release.
        }
        track.release()
      }
      audioTrack = null
      enqueuedFrames.set(0)
      interrupted = interruptedBySystem
      val manager = audioManager
      audioManager = null
      manager?.let {
        @Suppress("DEPRECATION")
        it.abandonAudioFocus(focusListener)
        it.mode = previousAudioMode
      }
    }
  }

  private fun status(): Map<String, Any> {
    return synchronized(lifecycleLock) {
      val track = audioTrack
      if (track == null) {
        mapOf(
          "started" to false,
          "queuedFrames" to 0,
          "playedMilliseconds" to 0,
          "interrupted" to interrupted,
        )
      } else {
        val playedFrames = track.playbackHeadPosition.toLong() and 0xffff_ffffL
        mapOf(
          "started" to true,
          "queuedFrames" to maxOf(0, enqueuedFrames.get() - playedFrames),
          "playedMilliseconds" to (playedFrames * 1_000 / REQUIRED_SAMPLE_RATE),
          "interrupted" to interrupted,
        )
      }
    }
  }
}
