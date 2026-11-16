package no.onstad.metronom

import android.content.pm.ApplicationInfo
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import uniffi.metronom_ffi.Metronome
import uniffi.metronom_ffi.SettingsRecord

/**
 * The manual metronome, all on one screen: the beat flash and dots, the tempo (tap the number to
 * type it, or use the slider), beats per bar, tap tempo and Start/Stop. **Save as song** opens the
 * new-song page, which shows the same metronome, so the tempo and beats you set come along.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun MetronomeScreen(
  metronome: Metronome,
  onStart: () -> Unit,
  onStop: () -> Unit,
  settings: SettingsRecord,
  onOpenSongs: () -> Unit,
  onOpenSetlists: () -> Unit,
  onOpenSettings: () -> Unit,
  onSaveAsSong: () -> Unit,
) {
  val playback = rememberPlayback(metronome, onStart, onStop)
  val running = playback.running
  val state = rememberMetronomeState(metronome)
  var diagnostics by remember { mutableStateOf("") }
  val debuggable = (LocalContext.current.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) != 0

  val beatState = rememberBeatState(metronome, running, settings.visualOffsetMs)

  // Keep the display awake while the click is running (the `keep_screen_on` setting).
  KeepScreenOn(running && settings.keepScreenOn)

  // Debug builds show the audio diagnostics under the Start button.
  LaunchedEffect(debuggable) {
    while (debuggable) {
      diagnostics = "${metronome.diagnostics()} | ${metronome.sound()} vol ${"%.2f".format(metronome.volume())}"
      delay(500)
    }
  }

  // The content is spread over the screen when it fits and scrolls when it does not (a large
  // font setting, a small phone), so nothing is ever cut off.
  BoxWithConstraints(Modifier.fillMaxSize().safeDrawingPadding()) {
    val available = maxHeight
    Column(
      modifier =
        Modifier.fillMaxWidth()
          .verticalScroll(rememberScrollState())
          .padding(horizontal = 24.dp, vertical = 12.dp)
          .heightIn(min = available - 24.dp),
      horizontalAlignment = Alignment.CenterHorizontally,
      verticalArrangement = Arrangement.SpaceBetween,
    ) {
      Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        // Whole buttons wrap onto a second line when the text size leaves no room for one row.
        FlowRow(
          Modifier.fillMaxWidth(),
          horizontalArrangement = Arrangement.SpaceBetween,
          verticalArrangement = Arrangement.Center,
        ) {
          TextButton(onClick = onOpenSongs) { Text(stringResource(R.string.songs)) }
          TextButton(onClick = onOpenSetlists) { Text(stringResource(R.string.setlists)) }
          TextButton(onClick = onOpenSettings) { Text(stringResource(R.string.settings)) }
        }
        Text(
          stringResource(R.string.mode_manual),
          style = MaterialTheme.typography.labelLarge,
          color = MaterialTheme.colorScheme.onSecondaryContainer,
          modifier =
            Modifier.clip(CircleShape)
              .background(MaterialTheme.colorScheme.secondaryContainer)
              .padding(horizontal = 16.dp, vertical = 4.dp),
        )
        Spacer(Modifier.height(14.dp))
        FlashBar(beatState)
        Spacer(Modifier.height(14.dp))
        BeatDots(state.beats, beatState)
      }

      TempoControls(state, Modifier.padding(vertical = 12.dp))

      BarControls(state, Modifier.padding(vertical = 12.dp)) {
        OutlinedButton(onClick = onSaveAsSong) { Text(stringResource(R.string.save_as_song)) }
      }

      Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        Button(onClick = playback::toggle, modifier = Modifier.fillMaxWidth().height(72.dp)) {
          Text(stringResource(if (running) R.string.stop else R.string.start), fontSize = 24.sp)
        }
        if (debuggable) {
          Spacer(Modifier.height(8.dp))
          Text(
            diagnostics,
            fontFamily = FontFamily.Monospace,
            fontSize = 11.sp,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
          )
        }
      }
    }
  }
}
