package no.onstad.metronom

import android.content.pm.ApplicationInfo
import android.util.Log
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.exp
import kotlin.math.roundToInt
import kotlinx.coroutines.delay
import uniffi.metronom_ffi.BeatState
import uniffi.metronom_ffi.Metronome

/** Beats shown as dots; a longer bar shows a "Beat n of m" label instead. */
private const val MAX_DOTS = 16

/** How quickly the flash fades after a beat becomes audible. */
private const val FLASH_DECAY_MS = 90f

/**
 * How bright the flash is for the beat being heard: full at the moment it is heard and decaying
 * quickly, with the accented first beat of the bar brighter than the others.
 */
private fun flashIntensity(state: BeatState?): Float {
  if (state == null) return 0f
  val strength = if (state.beat == 0u) 1f else 0.55f
  return strength * exp(-state.sinceMs / FLASH_DECAY_MS)
}

/**
 * The manual metronome, all on one screen: the beat flash and dots, the tempo (tap the number to
 * type it, or use the slider), beats per bar, tap tempo and Start/Stop.
 */
@Composable
fun MetronomeScreen(
  metronome: Metronome,
  onStart: () -> Unit,
  onStop: () -> Unit,
  onOpenSongs: () -> Unit,
  onOpenSetlists: () -> Unit,
) {
  var running by remember { mutableStateOf(metronome.isRunning()) }
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

  // What is being heard right now, asked once per display frame. `withFrameNanos` hands us the
  // frame time on the same monotonic clock the audio system uses, so the flash is locked to the
  // sound the speaker is playing, not to a timer. Only draw/layer code reads this state, so the
  // screen does not recompose every frame.
  val beatState = remember { mutableStateOf<BeatState?>(null) }
  LaunchedEffect(running) {
    if (!running) {
      beatState.value = null
      return@LaunchedEffect
    }
    var lastBeat: UInt? = null
    while (true) {
      withFrameNanos { now ->
        val state = metronome.visualState(now)
        beatState.value = state
        // Debug builds: when the displayed beat changes, log how old it already was on the first
        // frame that showed it. A correct pipeline keeps that between 0 and one frame.
        if (debuggable && state != null && state.beat != lastBeat) {
          lastBeat = state.beat
          Log.d("MetronomFlash", "frame=${now / 1_000} us beat=${state.beat} sinceMs=${"%.2f".format(state.sinceMs)}")
        }
      }
    }
  }

  // The Rust side is the source of truth; poll it so the screen follows the service.
  LaunchedEffect(metronome) {
    while (true) {
      running = metronome.isRunning()
      if (debuggable) diagnostics = metronome.diagnostics()
      delay(500)
    }
  }

  Column(
    modifier = Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = 24.dp, vertical = 12.dp),
    horizontalAlignment = Alignment.CenterHorizontally,
  ) {
    Box(Modifier.fillMaxWidth()) {
      TextButton(onClick = onOpenSongs, modifier = Modifier.align(Alignment.CenterStart)) {
        Text(stringResource(R.string.songs))
      }
      TextButton(onClick = onOpenSetlists, modifier = Modifier.align(Alignment.CenterEnd)) {
        Text(stringResource(R.string.setlists))
      }
      Text(
        stringResource(R.string.mode_manual),
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.onSecondaryContainer,
        modifier =
          Modifier.align(Alignment.Center)
            .clip(CircleShape)
            .background(MaterialTheme.colorScheme.secondaryContainer)
            .padding(horizontal = 16.dp, vertical = 4.dp),
      )
    }
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

/** A bar across the top that lights up with every audible beat. */
@Composable
private fun FlashBar(state: State<BeatState?>) {
  val color = MaterialTheme.colorScheme.primary
  Box(
    Modifier.fillMaxWidth().height(14.dp).clip(CircleShape).drawBehind {
      drawRect(color, alpha = flashIntensity(state.value))
    }
  )
}

/**
 * One dot per beat of the bar, the first one larger; the dot of the beat being heard swells with
 * the flash. A bar longer than [MAX_DOTS] beats shows a "Beat n of m" label instead.
 */
@Composable
private fun BeatDots(beats: Int, state: State<BeatState?>) {
  if (beats > MAX_DOTS) {
    val current by remember { derivedStateOf { (state.value?.beat?.toInt() ?: 0) + 1 } }
    Text(stringResource(R.string.beat_of, current, beats), style = MaterialTheme.typography.titleMedium)
    return
  }
  val normal = MaterialTheme.colorScheme.outline
  val accent = MaterialTheme.colorScheme.primary
  Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
    repeat(beats) { index ->
      val dotSize = if (index == 0) 22.dp else 15.dp
      Box(
        Modifier.graphicsLayer {
            val current = state.value
            val swell = if (current != null && current.beat == index.toUInt()) flashIntensity(current) else 0f
            scaleX = 1f + 0.4f * swell
            scaleY = scaleX
          }
          .size(dotSize)
          .clip(CircleShape)
          .background(if (index == 0) accent else normal)
      )
    }
  }
}
