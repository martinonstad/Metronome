package no.onstad.metronom

import android.Manifest
import android.content.Context
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
import androidx.compose.runtime.mutableIntStateOf
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

  override fun attachBaseContext(newBase: Context) {
    super.attachBaseContext(newBase.withDebugOverrides())
  }

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
    // The song tapped last in the setlist being played (-1: none yet); kept so the same setlist
    // still shows it when you come back.
    var gigStem by rememberSaveable { mutableStateOf<String?>(null) }
    var gigIndex by rememberSaveable { mutableIntStateOf(-1) }

    BackHandler(enabled = screen.parent != null) { screen = screen.parent ?: Screen.Manual }

    when (val current = screen) {
      Screen.Manual ->
        MetronomeScreen(
          metronome = metronome,
          onStart = ::onStartRequested,
          onStop = ::stopPlayback,
          settings = library.settings,
          onOpenSongs = { screen = Screen.Songs },
          onOpenSetlists = { screen = Screen.Setlists },
          onOpenSettings = { screen = Screen.Settings },
          onSaveAsSong = { screen = Screen.EditSong(null, fromManual = true) },
        )
      Screen.Settings ->
        SettingsScreen(
          store = library,
          metronome = metronome,
          onBack = { screen = Screen.Manual },
          onOpenAbout = { screen = Screen.About },
        )
      Screen.About -> AboutScreen(onBack = { screen = Screen.Settings })
      Screen.Setlists ->
        SetlistsScreen(
          store = library,
          onBack = { screen = Screen.Manual },
          onPlay = { stem ->
            if (stem != gigStem) {
              gigStem = stem
              gigIndex = -1
            }
            screen = Screen.Gig(stem)
          },
          onEdit = { screen = Screen.EditSetlist(it) },
        )
      is Screen.Gig ->
        GigScreen(
          metronome = metronome,
          store = library,
          stem = current.stem,
          selected = if (gigStem == current.stem) gigIndex else -1,
          onSelect = {
            gigStem = current.stem
            gigIndex = it
          },
          onStart = ::onStartRequested,
          onStop = ::stopPlayback,
          onBack = { screen = Screen.Manual },
          onEdit = { screen = Screen.EditSetlist(current.stem) },
        )
      is Screen.EditSetlist ->
        SetlistEditorScreen(
          store = library,
          stem = current.stem,
          onBack = { screen = Screen.Setlists },
          onOpen = { screen = Screen.EditSetlist(it) },
        )
      Screen.Songs ->
        SongsScreen(
          store = library,
          onBack = { screen = Screen.Manual },
          onOpen = { title ->
            // A song is edited on the live metronome, so opening it makes it the current tempo
            // and bar length (a running click switches on the next beat).
            title?.let(library::song)?.let(metronome::load)
            screen = Screen.EditSong(title)
          },
        )
      is Screen.EditSong ->
        SongEditorScreen(
          metronome = metronome,
          store = library,
          originalTitle = current.title,
          onStart = ::onStartRequested,
          onStop = ::stopPlayback,
          onClose = { screen = current.parent ?: Screen.Manual },
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
