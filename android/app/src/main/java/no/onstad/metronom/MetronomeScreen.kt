package no.onstad.metronom

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import uniffi.metronom_ffi.Metronome
import kotlin.math.roundToInt

/** Milestone 0 spike screen: just enough control to judge the audio on a real phone. */
@Composable
fun MetronomeScreen(metronome: Metronome, onStart: () -> Unit, onStop: () -> Unit) {
  var running by remember { mutableStateOf(metronome.isRunning()) }
  var bpm by remember { mutableStateOf(metronome.bpm()) }
  var beats by remember { mutableStateOf(metronome.beatsPerBar().toInt()) }
  var diagnostics by remember { mutableStateOf(metronome.diagnostics()) }

  // The Rust side is the source of truth; poll it so the screen follows the service.
  LaunchedEffect(metronome) {
    while (true) {
      running = metronome.isRunning()
      diagnostics = metronome.diagnostics()
      delay(500)
    }
  }

  fun changeBpm(delta: Double) {
    metronome.setBpm(metronome.bpm() + delta)
    bpm = metronome.bpm()
  }

  fun changeBeats(delta: Int) {
    metronome.setBeatsPerBar((metronome.beatsPerBar().toInt() + delta).coerceAtLeast(1).toUInt())
    beats = metronome.beatsPerBar().toInt()
  }

  Column(
    modifier = Modifier.fillMaxSize().safeDrawingPadding().padding(24.dp),
    horizontalAlignment = Alignment.CenterHorizontally,
    verticalArrangement = Arrangement.Center,
  ) {
    Text(bpm.roundToInt().toString(), fontSize = 96.sp, style = MaterialTheme.typography.displayLarge)
    Text(stringResource(R.string.bpm), style = MaterialTheme.typography.titleMedium)
    Spacer(Modifier.height(16.dp))
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
      for (delta in listOf(-5.0, -1.0, 1.0, 5.0)) {
        FilledTonalButton(onClick = { changeBpm(delta) }) {
          Text(if (delta > 0) "+${delta.toInt()}" else "−${-delta.toInt()}")
        }
      }
    }

    Spacer(Modifier.height(32.dp))
    Text(stringResource(R.string.beats_per_bar), style = MaterialTheme.typography.titleMedium)
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
      FilledTonalButton(onClick = { changeBeats(-1) }) { Text("−") }
      Text(beats.toString(), style = MaterialTheme.typography.headlineLarge)
      FilledTonalButton(onClick = { changeBeats(1) }) { Text("+") }
    }

    Spacer(Modifier.height(40.dp))
    Button(onClick = { if (running) onStop() else onStart() }, modifier = Modifier.fillMaxWidth().height(72.dp)) {
      Text(stringResource(if (running) R.string.stop else R.string.start), fontSize = 24.sp)
    }

    Spacer(Modifier.height(24.dp))
    Text(
      diagnostics,
      fontFamily = FontFamily.Monospace,
      fontSize = 11.sp,
      textAlign = TextAlign.Center,
      color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
  }
}
