package no.onstad.metronom

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.content.pm.ServiceInfo
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import uniffi.metronom_ffi.MetronomeException

/**
 * Foreground service that owns playback, so the click keeps going with the screen locked or the
 * activity gone. Start it with [android.content.Context.startForegroundService]; stop it with
 * [android.content.Context.stopService] or the notification's Stop action.
 */
class PlaybackService : Service() {
  private val handler = Handler(Looper.getMainLooper())
  private val logDiagnostics =
    object : Runnable {
      override fun run() {
        Log.i(TAG, metronomApp.metronome.diagnostics())
        handler.postDelayed(this, DIAGNOSTICS_INTERVAL_MS)
      }
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
    try {
      metronomApp.metronome.start(metronomApp.nativeSampleRate())
    } catch (e: MetronomeException) {
      Log.e(TAG, "Could not start audio", e)
      stopSelf()
      return
    }
    if (applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0) {
      handler.removeCallbacks(logDiagnostics)
      handler.postDelayed(logDiagnostics, DIAGNOSTICS_INTERVAL_MS)
    }
  }

  override fun onDestroy() {
    handler.removeCallbacksAndMessages(null)
    metronomApp.metronome.stop()
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
  }
}
