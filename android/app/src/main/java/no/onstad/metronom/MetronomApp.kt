package no.onstad.metronom

import android.app.Application
import android.content.Context
import android.media.AudioManager
import uniffi.metronom_ffi.Metronome

/** Holds the one [Metronome] shared by the UI and the playback service. */
class MetronomApp : Application() {
  val metronome: Metronome by lazy { Metronome() }

  /** The device's native output rate, so AAudio can use its low-latency path without resampling. */
  fun nativeSampleRate(): UInt {
    val audioManager = getSystemService(Context.AUDIO_SERVICE) as AudioManager
    return audioManager.getProperty(AudioManager.PROPERTY_OUTPUT_SAMPLE_RATE)?.toUIntOrNull() ?: 48_000u
  }
}

val Context.metronomApp: MetronomApp
  get() = applicationContext as MetronomApp
