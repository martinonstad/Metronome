package no.onstad.metronom

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
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
        Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) { App() }
      }
    }
  }

  /** The current screen, with Back leading to its parent (and out of the app from the home screen). */
  @Composable
  private fun App() {
    val metronome = metronomApp.metronome
    val library = metronomApp.library
    var screen: Screen by rememberSaveable(stateSaver = ScreenSaver) { mutableStateOf(Screen.Manual) }

    BackHandler(enabled = screen.parent != null) { screen = screen.parent ?: Screen.Manual }

    when (val current = screen) {
      Screen.Manual ->
        MetronomeScreen(
          metronome = metronome,
          onStart = ::onStartRequested,
          onStop = ::stopPlayback,
          onOpenSongs = { screen = Screen.Songs },
        )
      Screen.Songs ->
        SongsScreen(
          store = library,
          onBack = { screen = Screen.Manual },
          onOpen = { screen = Screen.EditSong(it) },
        )
      is Screen.EditSong ->
        SongEditorScreen(
          store = library,
          originalTitle = current.title,
          onClose = { screen = Screen.Songs },
          onPlay = { song ->
            // Load the song into the manual metronome; the new bar starts on the next beat.
            metronome.setBpm(song.bpm)
            metronome.setBeatsPerBar(song.beats)
            metronome.restartBar()
            screen = Screen.Manual
          },
        )
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
