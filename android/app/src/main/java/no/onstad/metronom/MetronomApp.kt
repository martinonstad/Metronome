package no.onstad.metronom

import android.app.Application
import android.content.Context
import android.media.AudioManager
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import uniffi.metronom_ffi.Metronome

/** Holds the one [Metronome] and the song library, shared by the UI and the playback service. */
class MetronomApp : Application() {
  val metronome: Metronome by lazy { Metronome() }

  /** Songs, setlists and settings, kept as markdown files in the app's private folder. */
  val library: LibraryStore by lazy {
    LibraryStore(File(filesDir, "Metronom"), CoroutineScope(SupervisorJob() + Dispatchers.Default))
  }

  override fun onCreate() {
    super.onCreate()
    library // start opening the library in the background right away
  }

  /** The device's native output rate, so AAudio can use its low-latency path without resampling. */
  fun nativeSampleRate(): UInt {
    val audioManager = getSystemService(Context.AUDIO_SERVICE) as AudioManager
    return audioManager.getProperty(AudioManager.PROPERTY_OUTPUT_SAMPLE_RATE)?.toUIntOrNull() ?: 48_000u
  }
}

val Context.metronomApp: MetronomApp
  get() = applicationContext as MetronomApp
