package no.onstad.metronom

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import uniffi.metronom_ffi.SetlistUseRecord
import uniffi.metronom_ffi.SongRecord

/**
 * Add a song or edit one: title, tempo, beats per bar and notes. [originalTitle] is the song
 * being edited, or `null` for a new one. Leaving without saving discards the changes.
 */
@Composable
fun SongEditorScreen(
  store: LibraryStore,
  originalTitle: String?,
  onClose: () -> Unit,
  onPlay: (SongRecord) -> Unit,
) {
  val original = remember(originalTitle) { originalTitle?.let { store.song(it) } }
  var title by rememberSaveable { mutableStateOf(original?.title ?: "") }
  var bpm by rememberSaveable { mutableStateOf(original?.bpm ?: 120.0) }
  var beats by rememberSaveable { mutableIntStateOf(original?.beats?.toInt() ?: 4) }
  var notes by rememberSaveable { mutableStateOf(original?.notes ?: "") }
  var titleError by remember { mutableStateOf<String?>(null) }
  var showTempoDialog by remember { mutableStateOf(false) }
  var deleteImpact by remember { mutableStateOf<List<SetlistUseRecord>?>(null) }
  var problem by remember { mutableStateOf<String?>(null) }
  val scope = rememberCoroutineScope()

  fun current() = SongRecord(title.trim(), bpm, beats.toUInt(), notes.trim())

  fun save() {
    if (title.isBlank()) {
      titleError = "Enter a title"
      return
    }
    scope.launch {
      val song = current()
      val outcome =
        if (original == null) store.change { it.addSong(song) } else store.change { it.updateSong(original.title, song) }
      when (outcome) {
        is Outcome.Ok -> onClose()
        is Outcome.Failed -> titleError = outcome.message
      }
    }
  }

  Column(
    Modifier.fillMaxSize().safeDrawingPadding().imePadding().verticalScroll(rememberScrollState()).padding(horizontal = 24.dp, vertical = 12.dp),
    verticalArrangement = Arrangement.spacedBy(16.dp),
  ) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
      TextButton(onClick = onClose) { Text("‹ ${stringResource(R.string.back)}") }
      Spacer(Modifier.weight(1f))
      Text(
        stringResource(if (original == null) R.string.new_song else R.string.edit_song),
        style = MaterialTheme.typography.titleMedium,
      )
    }

    OutlinedTextField(
      value = title,
      onValueChange = {
        title = it
        titleError = null
      },
      label = { Text(stringResource(R.string.title_label)) },
      singleLine = true,
      isError = titleError != null,
      supportingText = { titleError?.let { Text(it) } },
      modifier = Modifier.fillMaxWidth(),
    )

    Column(Modifier.fillMaxWidth()) {
      Text(stringResource(R.string.tempo_label), style = MaterialTheme.typography.labelLarge)
      val tempoDescription = stringResource(R.string.tempo_description, bpm.roundToInt())
      Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
          bpm.roundToInt().toString(),
          fontSize = 56.sp,
          style = MaterialTheme.typography.displayMedium,
          modifier = Modifier.padding(vertical = 4.dp).semantics { contentDescription = tempoDescription },
        )
        TextButton(onClick = { showTempoDialog = true }) { Text(stringResource(R.string.type_tempo)) }
      }
      val sliderDescription = stringResource(R.string.slider_description)
      Slider(
        value = bpm.toFloat().coerceIn(MIN_BPM.toFloat(), MAX_BPM.toFloat()),
        onValueChange = { bpm = it.roundToInt().toDouble() },
        valueRange = MIN_BPM.toFloat()..MAX_BPM.toFloat(),
        modifier = Modifier.fillMaxWidth().semantics { contentDescription = sliderDescription },
      )
    }

    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
      Text(stringResource(R.string.beats_per_bar), style = MaterialTheme.typography.bodyMedium)
      NumberStepper(value = beats, range = 1..MAX_BEATS, onChange = { beats = it })
    }

    OutlinedTextField(
      value = notes,
      onValueChange = { notes = it.replace('\n', ' ') },
      label = { Text(stringResource(R.string.notes_label)) },
      placeholder = { Text(stringResource(R.string.notes_hint)) },
      minLines = 2,
      maxLines = 3,
      modifier = Modifier.fillMaxWidth(),
    )

    problem?.let { Text(it, color = MaterialTheme.colorScheme.error) }

    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), modifier = Modifier.fillMaxWidth()) {
      if (original != null) {
        OutlinedButton(
          onClick = {
            scope.launch { deleteImpact = store.read { it.setlistsUsing(original.title) } ?: emptyList() }
          },
          modifier = Modifier.height(56.dp),
        ) {
          Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error)
        }
      }
      Button(onClick = { save() }, modifier = Modifier.weight(1f).height(56.dp)) { Text(stringResource(R.string.save)) }
    }
    OutlinedButton(onClick = { onPlay(current()) }, modifier = Modifier.fillMaxWidth().height(48.dp)) {
      Text(stringResource(R.string.play_now))
    }
    Spacer(Modifier.height(8.dp))
  }

  if (showTempoDialog) {
    TempoDialog(
      current = bpm.roundToInt(),
      onDismiss = { showTempoDialog = false },
      onConfirm = {
        bpm = it.toDouble()
        showTempoDialog = false
      },
    )
  }

  val impact = deleteImpact
  if (impact != null && original != null) {
    AlertDialog(
      onDismissRequest = { deleteImpact = null },
      title = { Text(stringResource(R.string.delete_song_title, original.title)) },
      text = {
        if (impact.isEmpty()) {
          Text(stringResource(R.string.delete_song_unused))
        } else {
          Text(
            stringResource(R.string.delete_song_used) +
              impact.joinToString("") { "\n• ${it.name} (×${it.count})" }
          )
        }
      },
      confirmButton = {
        TextButton(
          onClick = {
            deleteImpact = null
            scope.launch {
              when (val outcome = store.change { it.deleteSong(original.title) }) {
                is Outcome.Ok -> onClose()
                is Outcome.Failed -> problem = outcome.message
              }
            }
          }
        ) {
          Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error)
        }
      },
      dismissButton = { TextButton(onClick = { deleteImpact = null }) { Text(stringResource(R.string.cancel)) } },
    )
  }
}
