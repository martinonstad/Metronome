package no.onstad.metronom

import android.os.SystemClock
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.metronom_ffi.BeatState
import uniffi.metronom_ffi.Metronome

// The parts of the metronome view: the tempo and bar length as state, the controls that set
// them, and starting and stopping the click (also used by the list pages).

/**
 * The tempo and bar length of the shared [Metronome] as Compose state. The Rust side is the
 * source of truth: every change is written there and read back, so two screens that show the same
 * metronome always agree.
 */
@Stable
internal class MetronomeState(private val metronome: Metronome) {
  /** The tempo exactly as the metronome has it; a hand-written song can have decimals. */
  var exactBpm by mutableDoubleStateOf(metronome.bpm())
    private set

  var beats by mutableIntStateOf(metronome.beatsPerBar().toInt())
    private set

  /** The tempo as shown. */
  val bpm: Int
    get() = exactBpm.roundToInt()

  /** Read the metronome again, after something else changed it. */
  fun refresh() {
    exactBpm = metronome.bpm()
    beats = metronome.beatsPerBar().toInt()
  }

  fun setTempo(value: Int) {
    metronome.setBpm(value.coerceIn(MIN_BPM, MAX_BPM).toDouble())
    refresh()
  }

  fun setBeatsPerBar(value: Int) {
    metronome.setBeatsPerBar(value.coerceIn(1, MAX_BEATS).toUInt())
    refresh()
  }

  /**
   * A tap on the tap-tempo button. The measured tempo is rounded to a whole number, so the number
   * on the screen is the tempo that plays and the tempo that is saved.
   */
  fun tap() {
    metronome.tap(System.nanoTime())?.let { metronome.setBpm(it.roundToInt().toDouble()) }
    refresh()
  }
}

@Composable
internal fun rememberMetronomeState(metronome: Metronome): MetronomeState =
  remember(metronome) { MetronomeState(metronome) }

/**
 * Whether the click is running, and starting and stopping it. The Rust side is the source of
 * truth and is polled, but [running] flips the moment you ask, so a button or a row reacts at
 * once. If the engine never follows (a start refused during a call), it falls back after
 * [CONFIRM_TIMEOUT_MS]. A new request is ignored until the previous one is confirmed, so a quick
 * double tap cannot stop the playback service while it is still starting.
 */
@Stable
internal class Playback(
  private val metronome: Metronome,
  private val onStart: () -> Unit,
  private val onStop: () -> Unit,
) {
  private var polled by mutableStateOf(metronome.isRunning())
  private var wanted by mutableStateOf<Boolean?>(null)
  private var deadline = 0L
  private val wake = Channel<Unit>(Channel.CONFLATED)

  val running: Boolean
    get() = wanted ?: polled

  fun start() = request(true)

  fun stop() = request(false)

  fun toggle() = request(!running)

  private fun request(run: Boolean) {
    if (wanted != null || run == polled) return
    wanted = run
    deadline = SystemClock.elapsedRealtime() + CONFIRM_TIMEOUT_MS
    if (run) onStart() else onStop()
    wake.trySend(Unit)
  }

  /**
   * Follow the engine: slowly while nothing is asked for, and often from the moment a request is
   * made until it is confirmed.
   */
  internal suspend fun poll() {
    while (true) {
      polled = metronome.isRunning()
      if (wanted == polled || SystemClock.elapsedRealtime() > deadline) wanted = null
      withTimeoutOrNull(if (wanted != null) FAST_POLL_MS else SLOW_POLL_MS) { wake.receive() }
    }
  }

  private companion object {
    const val CONFIRM_TIMEOUT_MS = 2_000L
    const val FAST_POLL_MS = 50L
    const val SLOW_POLL_MS = 500L
  }
}

@Composable
internal fun rememberPlayback(metronome: Metronome, onStart: () -> Unit, onStop: () -> Unit): Playback {
  val start by rememberUpdatedState(onStart)
  val stop by rememberUpdatedState(onStop)
  val playback = remember(metronome) { Playback(metronome, onStart = { start() }, onStop = { stop() }) }
  LaunchedEffect(playback) { playback.poll() }
  return playback
}

/** Keep the display awake while [enabled] and this is on screen. */
@Composable
internal fun KeepScreenOn(enabled: Boolean) {
  val view = LocalView.current
  DisposableEffect(view, enabled) {
    view.keepScreenOn = enabled
    onDispose { view.keepScreenOn = false }
  }
}

/**
 * The tempo: the number ([numeral] sp; tap it to type a tempo), and the slider with − and + to
 * nudge it by one.
 */
@Composable
internal fun TempoControls(state: MetronomeState, modifier: Modifier = Modifier, numeral: Int = 96) {
  var showTempoDialog by remember { mutableStateOf(false) }
  Column(modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
    val tempoDescription = stringResource(R.string.tempo_description, state.bpm)
    Text(
      state.bpm.toString(),
      fontSize = numeralSize(numeral),
      style = MaterialTheme.typography.displayLarge,
      modifier =
        Modifier.clickable(role = Role.Button) { showTempoDialog = true }
          .semantics { contentDescription = tempoDescription },
    )
    Text(
      stringResource(R.string.bpm_hint),
      style = MaterialTheme.typography.bodySmall,
      textAlign = TextAlign.Center,
      color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Spacer(Modifier.height(16.dp))
    val sliderDescription = stringResource(R.string.slider_description)
    val slower = stringResource(R.string.slower)
    val faster = stringResource(R.string.faster)
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
      FilledTonalButton(
        onClick = { state.setTempo(state.bpm - 1) },
        modifier = Modifier.semantics { contentDescription = slower },
      ) {
        Text("−")
      }
      Slider(
        value = state.bpm.toFloat(),
        onValueChange = { state.setTempo(it.roundToInt()) },
        valueRange = MIN_BPM.toFloat()..MAX_BPM.toFloat(),
        modifier = Modifier.weight(1f).semantics { contentDescription = sliderDescription },
      )
      FilledTonalButton(
        onClick = { state.setTempo(state.bpm + 1) },
        modifier = Modifier.semantics { contentDescription = faster },
      ) {
        Text("+")
      }
    }
  }

  if (showTempoDialog) {
    TempoDialog(
      current = state.bpm,
      onDismiss = { showTempoDialog = false },
      onConfirm = {
        state.setTempo(it)
        showTempoDialog = false
      },
    )
  }
}

/**
 * The beats per bar: − and + on either side of the beat dots, and "4 beats" under them, so the
 * bar length is set where it is seen.
 */
@Composable
internal fun BeatsControl(state: MetronomeState, beatState: State<BeatState?>, modifier: Modifier = Modifier) {
  val fewer = stringResource(R.string.fewer_beats)
  val more = stringResource(R.string.more_beats)
  val count = pluralStringResource(R.plurals.beats_count, state.beats, state.beats)
  Column(modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
      FilledTonalButton(
        onClick = { state.setBeatsPerBar(state.beats - 1) },
        enabled = state.beats > 1,
        modifier = Modifier.semantics { contentDescription = fewer },
      ) {
        Text("−")
      }
      Box(Modifier.weight(1f).padding(horizontal = 8.dp), contentAlignment = Alignment.Center) {
        BeatDots(state.beats, beatState)
      }
      FilledTonalButton(
        onClick = { state.setBeatsPerBar(state.beats + 1) },
        enabled = state.beats < MAX_BEATS,
        modifier = Modifier.semantics { contentDescription = more },
      ) {
        Text("+")
      }
    }
    Text(
      count,
      style = MaterialTheme.typography.bodySmall,
      color = MaterialTheme.colorScheme.onSurfaceVariant,
      modifier = Modifier.padding(top = 4.dp),
    )
  }
}
