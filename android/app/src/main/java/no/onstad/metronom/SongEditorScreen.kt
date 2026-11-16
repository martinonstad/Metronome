package no.onstad.metronom

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import uniffi.metronom_ffi.BandGroup
import uniffi.metronom_ffi.LibraryException
import uniffi.metronom_ffi.Metronome
import uniffi.metronom_ffi.SetlistUseRecord
import uniffi.metronom_ffi.SongRecord

/**
 * Add a song or edit one, on the live metronome. The page is built like the main screen (flash,
 * the tempo number, −/+ and slider, beats per bar, tap tempo, Start/Stop), so a tempo is set to
 * the exact number and by ear, and adds the title, the notes and the setlists to put the song in.
 *
 * [originalTitle] is the song being edited, or `null` for a new one. A new song starts from
 * whatever the metronome is set to; the caller loads an existing song into the metronome before
 * opening the page. Leaving without saving changes no file.
 */
@Composable
fun SongEditorScreen(
  metronome: Metronome,
  store: LibraryStore,
  originalTitle: String?,
  onStart: () -> Unit,
  onStop: () -> Unit,
  onClose: () -> Unit,
) {
  // Only when the app was recreated on this page: the song cannot be looked up yet.
  if (originalTitle != null && store.state is LibraryStore.State.Opening) {
    Column(Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = 24.dp, vertical = 12.dp)) {
      TextButton(onClick = onClose) { Text("‹ ${stringResource(R.string.back)}") }
      Text(stringResource(R.string.opening_library), Modifier.padding(top = 24.dp))
    }
    return
  }

  val original = remember(originalTitle) { originalTitle?.let { store.song(it) } }
  val playback = rememberPlayback(metronome, onStart, onStop)
  val running = playback.running
  val state = rememberMetronomeState(metronome)
  val beatState = rememberBeatState(metronome, running, store.settings.visualOffsetMs)
  KeepScreenOn(running && store.settings.keepScreenOn)

  var title by rememberSaveable { mutableStateOf(original?.title ?: "") }
  var notes by rememberSaveable { mutableStateOf(original?.notes ?: "") }
  // The setlists (file names) the song is added to when it is saved.
  var picked by rememberSaveable { mutableStateOf(emptyList<String>()) }
  var titleError by remember { mutableStateOf<String?>(null) }
  var saving by remember { mutableStateOf(false) }
  var showSetlists by remember { mutableStateOf(false) }
  var notAdded by remember { mutableStateOf<List<String>?>(null) }
  var deleteImpact by remember { mutableStateOf<List<SetlistUseRecord>?>(null) }
  var problem by remember { mutableStateOf<String?>(null) }
  val scope = rememberCoroutineScope()

  // The tempo and beats this page last showed. The metronome keeps them through a rotation, but
  // a new process starts with a fresh metronome; then they are put back from here, so the page
  // never shows (and saves) a tempo you did not set.
  var draftBpm by rememberSaveable { mutableStateOf<Double?>(null) }
  var draftBeats by rememberSaveable { mutableIntStateOf(0) }
  LaunchedEffect(Unit) {
    val bpm = draftBpm
    if (bpm != null && (metronome.bpm() != bpm || metronome.beatsPerBar().toInt() != draftBeats)) {
      metronome.setBpm(bpm)
      metronome.setBeatsPerBar(draftBeats.toUInt())
      state.refresh()
    }
    snapshotFlow { state.exactBpm to state.beats }
      .collect { (exactBpm, beats) ->
        draftBpm = exactBpm
        draftBeats = beats
      }
  }

  // Where the song is used already (an existing song only).
  var uses by remember { mutableStateOf<List<SetlistUseRecord>>(emptyList()) }
  LaunchedEffect(original, store.revision) {
    uses = original?.let { song -> store.read { it.setlistsUsing(song.title) } } ?: emptyList()
  }

  val setlists = store.setlistGroups.flatMap { it.setlists }
  fun nameOf(stem: String) = setlists.firstOrNull { it.stem == stem }?.name ?: stem

  fun save() {
    if (saving) return
    if (title.isBlank()) {
      titleError = "Enter a title"
      return
    }
    saving = true
    val song = SongRecord(title.trim(), state.exactBpm, state.beats.toUInt(), notes.trim())
    val targets = picked.filter { stem -> setlists.any { it.stem == stem } }.associateWith(::nameOf)
    scope.launch {
      // One change: the song, then the setlists. If the song cannot be saved nothing is written;
      // a setlist that cannot be changed is reported and does not undo the song.
      val outcome =
        store.change { library ->
          if (original == null) library.addSong(song) else library.updateSong(original.title, song)
          targets.mapNotNull { (stem, name) ->
            try {
              library.addSongToSetlist(stem, song.title)
              null
            } catch (e: LibraryException) {
              "$name: ${describe(e)}"
            }
          }
        }
      when (outcome) {
        is Outcome.Failed -> {
          titleError = outcome.message
          saving = false
        }
        is Outcome.Ok -> if (outcome.value.isEmpty()) onClose() else notAdded = outcome.value
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

    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
      FlashBar(beatState)
      Spacer(Modifier.height(14.dp))
      BeatDots(state.beats, beatState)
    }

    TempoControls(state, numeral = 72)

    BarControls(state)

    FilledTonalButton(onClick = playback::toggle, modifier = Modifier.fillMaxWidth().height(56.dp)) {
      Text(stringResource(if (running) R.string.stop else R.string.start), fontSize = 20.sp)
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

    Column(Modifier.fillMaxWidth()) {
      Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(stringResource(R.string.setlists), style = MaterialTheme.typography.labelLarge, modifier = Modifier.weight(1f))
        TextButton(onClick = { showSetlists = true }) { Text(stringResource(R.string.add_to_setlist)) }
      }
      if (uses.isNotEmpty()) {
        Text(
          stringResource(R.string.song_in_setlists, uses.joinToString { if (it.count > 1u) "${it.name} (×${it.count})" else it.name }),
          style = MaterialTheme.typography.bodyMedium,
          color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
      }
      if (picked.isNotEmpty()) {
        Text(
          stringResource(R.string.song_will_be_added, picked.joinToString(transform = ::nameOf)),
          style = MaterialTheme.typography.bodyMedium,
          color = MaterialTheme.colorScheme.primary,
        )
      }
    }

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
    Spacer(Modifier.height(8.dp))
  }

  if (showSetlists) {
    PickSetlistsDialog(
      groups = store.setlistGroups,
      picked = picked,
      uses = uses,
      onToggle = { stem -> picked = if (stem in picked) picked - stem else picked + stem },
      onDone = { showSetlists = false },
    )
  }

  // The song is saved by now, so the only way on is out of the page.
  notAdded?.let { lines ->
    AlertDialog(
      onDismissRequest = onClose,
      title = { Text(stringResource(R.string.song_saved)) },
      text = { Text(stringResource(R.string.song_not_added) + lines.joinToString("") { "\n• $it" }) },
      confirmButton = { TextButton(onClick = onClose) { Text(stringResource(R.string.ok)) } },
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

/**
 * Tick the setlists the song is added to (at the end of each) when it is saved. A setlist that
 * has the song already says so; adding it again puts it in twice.
 */
@Composable
private fun PickSetlistsDialog(
  groups: List<BandGroup>,
  picked: List<String>,
  uses: List<SetlistUseRecord>,
  onToggle: (stem: String) -> Unit,
  onDone: () -> Unit,
) {
  AlertDialog(
    onDismissRequest = onDone,
    title = { Text(stringResource(R.string.add_to_setlist)) },
    text = {
      if (groups.isEmpty()) {
        Text(stringResource(R.string.no_setlists_yet))
      } else {
        LazyColumn(Modifier.heightIn(max = 360.dp)) {
          for (group in groups) {
            item(key = "band:${group.band}") {
              Text(
                group.band ?: stringResource(R.string.no_band),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(top = 12.dp, bottom = 4.dp),
              )
            }
            items(group.setlists, key = { it.stem }) { setlist ->
              val checked = setlist.stem in picked
              val already = uses.firstOrNull { it.stem == setlist.stem }?.count?.toInt() ?: 0
              Row(
                Modifier.fillMaxWidth()
                  .toggleable(value = checked, role = Role.Checkbox, onValueChange = { onToggle(setlist.stem) })
                  .heightIn(min = 48.dp)
                  .padding(vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
              ) {
                Checkbox(checked = checked, onCheckedChange = null)
                Spacer(Modifier.width(12.dp))
                Column {
                  Text(setlist.name, style = MaterialTheme.typography.bodyLarge)
                  if (already > 0) {
                    Text(
                      stringResource(R.string.in_setlist_times, already),
                      style = MaterialTheme.typography.bodySmall,
                      color = MaterialTheme.colorScheme.primary,
                    )
                  }
                }
              }
            }
          }
        }
      }
    },
    confirmButton = { TextButton(onClick = onDone) { Text(stringResource(R.string.done)) } },
  )
}
