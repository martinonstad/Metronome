package no.onstad.metronom

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import uniffi.metronom_ffi.Metronome
import uniffi.metronom_ffi.SettingsRecord
import uniffi.metronom_ffi.Sound

/** The flash can be moved this far either way (the range `settings.md` allows). */
private const val MAX_OFFSET_MS = 500

/**
 * Sound, volume, flash timing and keep-awake, and export/import of your files. Every change is
 * saved at once, like everywhere else in the app. A slider changes the click while you drag it
 * and is saved when you let go.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SettingsScreen(store: LibraryStore, metronome: Metronome, onBack: () -> Unit, onOpenAbout: () -> Unit) {
  val scope = rememberCoroutineScope()
  val settings = store.settings
  var message by remember { mutableStateOf<String?>(null) }
  var volume by remember(settings.volume) { mutableFloatStateOf(settings.volume) }
  var offset by remember(settings.visualOffsetMs) { mutableFloatStateOf(settings.visualOffsetMs.toFloat()) }

  fun save(transform: (SettingsRecord) -> SettingsRecord) {
    scope.launch {
      (store.updateSettings(transform) as? Outcome.Failed)?.let { message = it.message }
    }
  }

  Column(
    Modifier.fillMaxSize().safeDrawingPadding().verticalScroll(rememberScrollState()).padding(horizontal = 24.dp, vertical = 12.dp),
    verticalArrangement = Arrangement.spacedBy(16.dp),
  ) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
      TextButton(onClick = onBack) { Text("‹ ${stringResource(R.string.back)}") }
      Spacer(Modifier.weight(1f))
      Text(stringResource(R.string.settings), style = MaterialTheme.typography.titleMedium)
    }

    when (val state = store.state) {
      is LibraryStore.State.Opening -> Text(stringResource(R.string.opening_library))
      is LibraryStore.State.Failed -> Text(state.message, color = MaterialTheme.colorScheme.error)
      is LibraryStore.State.Ready -> {
        SectionTitle(R.string.settings_click)
        Text(stringResource(R.string.settings_sound), style = MaterialTheme.typography.bodyMedium)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
          for (sound in Sound.entries) {
            FilterChip(
              selected = settings.sound == sound,
              onClick = { save { it.copy(sound = sound) } },
              label = { Text(soundName(sound)) },
            )
          }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
          Text(stringResource(R.string.settings_volume), style = MaterialTheme.typography.bodyMedium)
          Spacer(Modifier.weight(1f))
          Text("${(volume * 100).roundToInt()} %", style = MaterialTheme.typography.bodyMedium)
        }
        val volumeDescription = stringResource(R.string.settings_volume)
        Slider(
          value = volume,
          onValueChange = {
            volume = (it * 100).roundToInt() / 100f // whole percent, so the file reads `0.5`, not `0.49887767`
            metronome.setVolume(volume) // heard at once if the click is running
          },
          onValueChangeFinished = { save { s -> s.copy(volume = volume) } },
          modifier = Modifier.fillMaxWidth().semantics { contentDescription = volumeDescription },
        )
        Text(
          stringResource(R.string.settings_click_hint),
          style = MaterialTheme.typography.bodySmall,
          color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(verticalAlignment = Alignment.CenterVertically) {
          Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.settings_mix), style = MaterialTheme.typography.bodyLarge)
            Text(
              stringResource(R.string.settings_mix_hint),
              style = MaterialTheme.typography.bodySmall,
              color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
          }
          Switch(
            checked = settings.mixWithOtherAudio,
            onCheckedChange = { on -> save { it.copy(mixWithOtherAudio = on) } },
          )
        }

        HorizontalDivider()
        SectionTitle(R.string.settings_flash)
        Row(verticalAlignment = Alignment.CenterVertically) {
          Text(stringResource(R.string.settings_offset), style = MaterialTheme.typography.bodyMedium)
          Spacer(Modifier.weight(1f))
          Text(offsetLabel(offset.roundToInt()), style = MaterialTheme.typography.bodyMedium)
          TextButton(
            onClick = { save { it.copy(visualOffsetMs = 0) } },
            enabled = settings.visualOffsetMs != 0 || offset != 0f,
          ) {
            Text(stringResource(R.string.reset))
          }
        }
        val offsetDescription = stringResource(R.string.settings_offset)
        Slider(
          value = offset,
          onValueChange = { offset = (it / 10).roundToInt() * 10f },
          onValueChangeFinished = { save { s -> s.copy(visualOffsetMs = offset.roundToInt()) } },
          valueRange = -MAX_OFFSET_MS.toFloat()..MAX_OFFSET_MS.toFloat(),
          modifier = Modifier.fillMaxWidth().semantics { contentDescription = offsetDescription },
        )
        Text(
          stringResource(R.string.settings_offset_hint),
          style = MaterialTheme.typography.bodySmall,
          color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        HorizontalDivider()
        SectionTitle(R.string.settings_screen)
        Row(verticalAlignment = Alignment.CenterVertically) {
          Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.settings_keep_awake), style = MaterialTheme.typography.bodyLarge)
            Text(
              stringResource(R.string.settings_keep_awake_hint),
              style = MaterialTheme.typography.bodySmall,
              color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
          }
          Switch(checked = settings.keepScreenOn, onCheckedChange = { on -> save { it.copy(keepScreenOn = on) } })
        }

        HorizontalDivider()
        SectionTitle(R.string.settings_files)
        FilesSection(store, onMessage = { message = it })

        HorizontalDivider()
        SectionTitle(R.string.about)
        OutlinedButton(onClick = onOpenAbout, modifier = Modifier.fillMaxWidth().height(48.dp)) {
          Text(stringResource(R.string.about_open))
        }
        Spacer(Modifier.height(8.dp))
      }
    }
  }

  message?.let { MessageDialog(it) { message = null } }
}

/** What happened, in a dialog: a message at the bottom of a scrolling screen is easy to miss. */
@Composable
private fun MessageDialog(text: String, onDismiss: () -> Unit) {
  AlertDialog(
    onDismissRequest = onDismiss,
    text = { Text(text) },
    confirmButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.ok)) } },
  )
}

@Composable
private fun SectionTitle(text: Int) {
  Text(stringResource(text), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
}

@Composable
private fun soundName(sound: Sound): String =
  stringResource(
    when (sound) {
      Sound.CLICK -> R.string.sound_click
      Sound.WOOD -> R.string.sound_wood
      Sound.BEEP -> R.string.sound_beep
      Sound.RIM -> R.string.sound_rim
    }
  )

/** "+120 ms", "−40 ms" or "0 ms". */
internal fun offsetLabel(ms: Int): String =
  when {
    ms > 0 -> "+$ms ms"
    ms < 0 -> "−${-ms} ms"
    else -> "0 ms"
  }
