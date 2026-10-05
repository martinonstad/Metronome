package no.onstad.metronom

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ApplicationInfo
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.BatteryManager
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.util.Log
import android.widget.Toast
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import java.io.File
import java.time.LocalDateTime
import uniffi.metronom_ffi.MetronomeException

/**
 * Foreground service that owns playback, so the click keeps going with the screen locked or the
 * activity gone. Start it with [android.content.Context.startForegroundService]; stop it with
 * [android.content.Context.stopService] or the notification's Stop action.
 *
 * It also ends itself, and so removes its notification, whenever the click cannot go on:
 * - another app or a call takes the audio (audio focus is lost; not when the "play together
 *   with other audio" setting is on, which never takes the focus),
 * - the output is about to change under you (headphones unplugged: "becoming noisy"),
 * - the audio device went away or the engine stopped for any other reason.
 * It never starts again by itself; you press Start.
 */
class PlaybackService : Service() {
  private val handler = Handler(Looper.getMainLooper())
  private val audioManager by lazy { getSystemService(AudioManager::class.java) }
  private var focusRequest: AudioFocusRequest? = null
  private var noisyReceiver: BroadcastReceiver? = null
  private val isDebuggable get() = applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
  private var tick = 0
  private val logDiagnostics =
    object : Runnable {
      override fun run() {
        val metronome = metronomApp.metronome
        val latency = metronome.outputLatencyMs(System.nanoTime())
        val line = "${metronome.diagnostics()}, latency ${latency?.let { "%.0f".format(it) } ?: "?"} ms"
        Log.i(TAG, line)
        if (tick++ % FILE_EVERY_N_TICKS == 0) appendToDiagnosticsFile(line)
        handler.postDelayed(this, DIAGNOSTICS_INTERVAL_MS)
      }
    }

  /**
   * Debug builds only: a timestamped record that survives the small log buffer and an unplugged
   * USB cable, including whether the phone was in Doze, interactive or charging, and the battery
   * level (whole percent, so only a rough indication of drain). Read it with
   * `adb shell run-as no.onstad.metronom cat files/diagnostics.log`.
   */
  private fun appendToDiagnosticsFile(line: String) {
    val power = getSystemService(PowerManager::class.java)
    val battery = getSystemService(BatteryManager::class.java)
    val level = battery.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY)
    val file = File(filesDir, DIAGNOSTICS_FILE)
    if (file.length() > MAX_DIAGNOSTICS_FILE_BYTES) file.delete()
    file.appendText(
      "${LocalDateTime.now().withNano(0)} | doze=${power.isDeviceIdleMode} " +
        "interactive=${power.isInteractive} charging=${battery.isCharging} battery=$level% | $line\n"
    )
  }

  /** Ends the service when the engine is no longer running, for instance after a device disconnect. */
  private val watchEngine =
    object : Runnable {
      override fun run() {
        if (!metronomApp.metronome.isRunning()) {
          Log.w(TAG, "The engine stopped (audio device gone?); ending the service")
          stopSelf()
          return
        }
        handler.postDelayed(this, WATCH_INTERVAL_MS)
      }
    }

  /** Another app or a call took the audio: stop; the click never restarts by itself. */
  private val onFocusChange =
    AudioManager.OnAudioFocusChangeListener { change ->
      if (change == AudioManager.AUDIOFOCUS_LOSS || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT) {
        Log.i(TAG, "Audio focus lost ($change); stopping")
        stopSelf()
      }
      // A short interruption that only asks us to be quieter is handled by the system.
    }

  /** Takes the audio focus. `false` when it is refused, which happens during a call. */
  private fun requestAudioFocus(): Boolean {
    val request =
      AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
        .setAudioAttributes(
          AudioAttributes.Builder()
            .setUsage(AudioAttributes.USAGE_MEDIA)
            .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
            .build()
        )
        .setOnAudioFocusChangeListener(onFocusChange, handler)
        .build()
    focusRequest = request
    return audioManager.requestAudioFocus(request) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
  }

  private fun listenForBecomingNoisy() {
    if (noisyReceiver != null) return
    val receiver =
      object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
          Log.i(TAG, "Audio output is becoming noisy (headphones unplugged?); stopping")
          stopSelf()
        }
      }
    ContextCompat.registerReceiver(
      this,
      receiver,
      IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
      ContextCompat.RECEIVER_NOT_EXPORTED,
    )
    noisyReceiver = receiver
  }

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    if (intent?.action == ACTION_STOP) {
      stopSelf()
      return START_NOT_STICKY
    }
    startPlayback()
    return START_NOT_STICKY
  }

  private fun startPlayback() {
    // A foreground service must call startForeground promptly after being started.
    ServiceCompat.startForeground(
      this,
      NOTIFICATION_ID,
      buildNotification(),
      ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK,
    )
    // Mixing with other audio (a setting) means not taking the focus at all: other apps keep
    // playing, and a call or another app does not stop the click.
    if (!metronomApp.library.settings.mixWithOtherAudio && !requestAudioFocus()) {
      Log.w(TAG, "Audio focus was refused; not starting")
      Toast.makeText(this, R.string.start_refused, Toast.LENGTH_LONG).show()
      stopSelf()
      return
    }
    try {
      metronomApp.metronome.start(metronomApp.nativeSampleRate())
    } catch (e: MetronomeException) {
      Log.e(TAG, "Could not start audio", e)
      Toast.makeText(this, R.string.start_failed, Toast.LENGTH_LONG).show()
      stopSelf()
      return
    }
    listenForBecomingNoisy()
    handler.removeCallbacks(watchEngine)
    handler.postDelayed(watchEngine, WATCH_INTERVAL_MS)
    if (isDebuggable) {
      tick = 0
      handler.removeCallbacks(logDiagnostics)
      handler.postDelayed(logDiagnostics, DIAGNOSTICS_INTERVAL_MS)
    }
  }

  override fun onDestroy() {
    handler.removeCallbacksAndMessages(null)
    // A run that ends without this line means the system killed the service.
    if (isDebuggable) appendToDiagnosticsFile("service destroyed: ${metronomApp.metronome.diagnostics()}")
    metronomApp.metronome.stop()
    noisyReceiver?.let { unregisterReceiver(it) }
    noisyReceiver = null
    focusRequest?.let { audioManager.abandonAudioFocusRequest(it) }
    focusRequest = null
    super.onDestroy()
  }

  private fun buildNotification(): Notification {
    val manager = getSystemService(NotificationManager::class.java)
    manager.createNotificationChannel(
      NotificationChannel(CHANNEL_ID, getString(R.string.playback_channel), NotificationManager.IMPORTANCE_LOW)
    )
    val open =
      PendingIntent.getActivity(
        this,
        0,
        Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
        PendingIntent.FLAG_IMMUTABLE,
      )
    val stop =
      PendingIntent.getService(
        this,
        1,
        Intent(this, PlaybackService::class.java).setAction(ACTION_STOP),
        PendingIntent.FLAG_IMMUTABLE,
      )
    return NotificationCompat.Builder(this, CHANNEL_ID)
      .setSmallIcon(R.drawable.ic_stat_metronome)
      .setContentTitle(getString(R.string.app_name))
      .setContentText(getString(R.string.playing))
      .setContentIntent(open)
      .setOngoing(true)
      .setOnlyAlertOnce(true)
      .setCategory(NotificationCompat.CATEGORY_TRANSPORT)
      .addAction(0, getString(R.string.stop), stop)
      .build()
  }

  companion object {
    const val ACTION_STOP = "no.onstad.metronom.action.STOP"
    private const val TAG = "Metronom"
    private const val CHANNEL_ID = "playback"
    private const val NOTIFICATION_ID = 1
    private const val DIAGNOSTICS_INTERVAL_MS = 2_000L
    private const val WATCH_INTERVAL_MS = 1_000L
    private const val FILE_EVERY_N_TICKS = 5 // one file line every 10 s
    private const val DIAGNOSTICS_FILE = "diagnostics.log"
    private const val MAX_DIAGNOSTICS_FILE_BYTES = 512 * 1024L
  }
}
