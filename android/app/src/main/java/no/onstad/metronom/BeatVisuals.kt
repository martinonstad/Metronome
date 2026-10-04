package no.onstad.metronom

import android.content.pm.ApplicationInfo
import android.util.Log
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlin.math.exp
import kotlinx.coroutines.delay
import uniffi.metronom_ffi.BeatState
import uniffi.metronom_ffi.Metronome
import uniffi.metronom_ffi.SongRecord

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

/** Make [song] the current tempo and bar length; a running click switches on the next beat, which becomes beat 1. */
internal fun Metronome.load(song: SongRecord) {
  setBpm(song.bpm)
  setBeatsPerBar(song.beats)
  restartBar()
}

/**
 * Whether the click is running. The Rust side is the source of truth, so this polls it and the
 * screen follows the foreground service.
 */
@Composable
internal fun rememberRunning(metronome: Metronome): State<Boolean> {
  val running = remember { mutableStateOf(metronome.isRunning()) }
  LaunchedEffect(metronome) {
    while (true) {
      running.value = metronome.isRunning()
      delay(500)
    }
  }
  return running
}

/**
 * What is being heard right now, asked once per display frame. `withFrameNanos` hands us the
 * frame time on the same monotonic clock the audio system uses, so the flash is locked to the
 * sound the speaker is playing, not to a timer. Only draw/layer code should read this state, so
 * the screen does not recompose every frame.
 */
@Composable
internal fun rememberBeatState(metronome: Metronome, running: Boolean): State<BeatState?> {
  val beatState = remember { mutableStateOf<BeatState?>(null) }
  val debuggable = (LocalContext.current.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) != 0
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
  return beatState
}

/** A bar across the top that lights up with every audible beat. */
@Composable
internal fun FlashBar(state: State<BeatState?>) {
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
internal fun BeatDots(beats: Int, state: State<BeatState?>) {
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
