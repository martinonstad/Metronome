package no.onstad.metronom

import android.content.pm.ApplicationInfo
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.roundToInt
import kotlinx.coroutines.delay
import uniffi.metronom_ffi.Metronome
import uniffi.metronom_ffi.SettingsRecord

/**
 * The manual metronome, all on one screen: the beat flash and dots, the tempo (tap the number to
 * type it, or use the slider), beats per bar, tap tempo and Start/Stop.
 */
@Composable
fun MetronomeScreen(
  metronome: Metronome,
  onStart: () -> Unit,
  onStop: () -> Unit,
  settings: SettingsRecord,
  onOpenSongs: () -> Unit,
  onOpenSetlists: () -> Unit,
  onOpenSettings: () -> Unit,
) {
  val runningState = rememberRunning(metronome)
  val running by runningState
  var bpm by remember { mutableIntStateOf(metronome.bpm().roundToInt()) }
  var beats by remember { mutableIntStateOf(metronome.beatsPerBar().toInt()) }
  var showTempoDialog by remember { mutableStateOf(false) }
  var diagnostics by remember { mutableStateOf("") }
  val debuggable = (LocalContext.current.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) != 0

  fun refresh() {
    bpm = metronome.bpm().roundToInt()
    beats = metronome.beatsPerBar().toInt()
  }

  fun setTempo(value: Int) {
    metronome.setBpm(value.coerceIn(MIN_BPM, MAX_BPM).toDouble())
    refresh()
  }

  val beatState = rememberBeatState(metronome, running, settings.visualOffsetMs)

  // Keep the display awake while the click is running (the `keep_screen_on` setting).
  val view = LocalView.current
  val keepAwake = running && settings.keepScreenOn
  DisposableEffect(keepAwake) {
    view.keepScreenOn = keepAwake
    onDispose { view.keepScreenOn = false }
  }

  // Debug builds show the audio diagnostics under the Start button.
  LaunchedEffect(debuggable) {
    while (debuggable) {
      diagnostics = "${metronome.diagnostics()} | ${metronome.sound()} vol ${"%.2f".format(metronome.volume())}"
      delay(500)
    }
  }

  Column(
    modifier = Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = 24.dp, vertical = 12.dp),
    horizontalAlignment = Alignment.CenterHorizontally,
  ) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
      TextButton(onClick = onOpenSongs) { Text(stringResource(R.string.songs)) }
      TextButton(onClick = onOpenSetlists) { Text(stringResource(R.string.setlists)) }
      Spacer(Modifier.weight(1f))
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
    BeatDots(beats, beatState)

    Spacer(Modifier.weight(1f))
    val tempoDescription = stringResource(R.string.tempo_description, bpm)
    Text(
      bpm.toString(),
      fontSize = 96.sp,
      style = MaterialTheme.typography.displayLarge,
      modifier =
        Modifier.clickable { showTempoDialog = true }.semantics { contentDescription = tempoDescription },
    )
    Text(
      stringResource(R.string.bpm_hint),
      style = MaterialTheme.typography.bodySmall,
      color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Spacer(Modifier.height(16.dp))
    val sliderDescription = stringResource(R.string.slider_description)
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
      FilledTonalButton(onClick = { setTempo(bpm - 1) }) { Text("−") }
      Slider(
        value = bpm.toFloat(),
        onValueChange = { setTempo(it.roundToInt()) },
        valueRange = MIN_BPM.toFloat()..MAX_BPM.toFloat(),
        modifier = Modifier.weight(1f).semantics { contentDescription = sliderDescription },
      )
      FilledTonalButton(onClick = { setTempo(bpm + 1) }) { Text("+") }
    }

    Spacer(Modifier.weight(1f))
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
      Text(stringResource(R.string.beats_per_bar), style = MaterialTheme.typography.bodyMedium)
      FilledTonalButton(
        onClick = {
          metronome.setBeatsPerBar((beats - 1).coerceAtLeast(1).toUInt())
          refresh()
        }
      ) {
        Text("−")
      }
      Text(beats.toString(), style = MaterialTheme.typography.headlineMedium)
      FilledTonalButton(
        onClick = {
          metronome.setBeatsPerBar((beats + 1).coerceAtMost(MAX_BEATS).toUInt())
          refresh()
        }
      ) {
        Text("+")
      }
    }
    Spacer(Modifier.height(8.dp))
    OutlinedButton(
      onClick = {
        metronome.tap(System.nanoTime())
        refresh()
      }
    ) {
      Text(stringResource(R.string.tap))
    }

    Spacer(Modifier.weight(1f))
    Button(onClick = { if (running) onStop() else onStart() }, modifier = Modifier.fillMaxWidth().height(72.dp)) {
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

  if (showTempoDialog) {
    TempoDialog(
      current = bpm,
      onDismiss = { showTempoDialog = false },
      onConfirm = {
        setTempo(it)
        showTempoDialog = false
      },
    )
  }
}
