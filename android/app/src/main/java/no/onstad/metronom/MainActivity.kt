package no.onstad.metronom

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import androidx.core.content.ContextCompat
import no.onstad.metronom.theme.MetronomTheme

class MainActivity : ComponentActivity() {
  // The notification permission only decides whether the "playing" notification is visible;
  // playback starts either way.
  private val notificationPermission =
    registerForActivityResult(ActivityResultContracts.RequestPermission()) { startPlayback() }

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)

    enableEdgeToEdge()
    setContent {
      MetronomTheme {
        Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
          MetronomeScreen(metronome = metronomApp.metronome, onStart = ::onStartRequested, onStop = ::stopPlayback)
        }
      }
    }
  }

  private fun onStartRequested() {
    val needsPermission =
      Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
        ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) !=
          PackageManager.PERMISSION_GRANTED
    if (needsPermission) {
      notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
    } else {
      startPlayback()
    }
  }

  private fun startPlayback() {
    ContextCompat.startForegroundService(this, Intent(this, PlaybackService::class.java))
  }

  private fun stopPlayback() {
    stopService(Intent(this, PlaybackService::class.java))
  }
}
