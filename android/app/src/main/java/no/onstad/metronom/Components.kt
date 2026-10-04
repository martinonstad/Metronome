package no.onstad.metronom

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The tempo range everywhere in the app (the engine clamps to the same range). */
internal const val MIN_BPM = 30
internal const val MAX_BPM = 300

/** Beats per bar, 1–99. */
internal const val MAX_BEATS = 99

/** Type a tempo directly. Rejects anything outside 30–300 with a message instead of clamping. */
@Composable
internal fun TempoDialog(current: Int, onDismiss: () -> Unit, onConfirm: (Int) -> Unit) {
  var text by remember { mutableStateOf("") }
  var invalid by remember { mutableStateOf(false) }
  val focus = remember { FocusRequester() }
  val confirm = {
    val value = text.toIntOrNull()
    if (value == null || value !in MIN_BPM..MAX_BPM) invalid = true else onConfirm(value)
  }
  AlertDialog(
    onDismissRequest = onDismiss,
    title = { Text(stringResource(R.string.enter_tempo)) },
    text = {
      OutlinedTextField(
        value = text,
        onValueChange = {
          text = it.filter(Char::isDigit).take(3)
          invalid = false
        },
        placeholder = { Text(current.toString()) },
        singleLine = true,
        isError = invalid,
        supportingText = { if (invalid) Text(stringResource(R.string.tempo_error)) },
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { confirm() }),
        modifier = Modifier.focusRequester(focus),
      )
    },
    confirmButton = { TextButton(onClick = { confirm() }) { Text(stringResource(R.string.ok)) } },
    dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
  )
  LaunchedEffect(Unit) { focus.requestFocus() }
}

/**
 * The size of a large display numeral: [base] sp, but never smaller than [base] dp and never more
 * than [maxScale] times that, whatever the phone's font-size setting (Android 14 and later scale
 * big text less than small text; older versions scale it linearly). The numbers are already
 * large and the screen must still fit.
 */
@Composable
internal fun numeralSize(base: Int, maxScale: Float = 1.3f): TextUnit =
  with(LocalDensity.current) {
    val plain = base.dp.toPx()
    base.sp.toPx().coerceIn(plain, plain * maxScale).toDp().toSp()
  }

/** A − / number / + control for whole numbers in `range`; the buttons say what they do to a screen reader. */
@Composable
internal fun NumberStepper(value: Int, range: IntRange, onChange: (Int) -> Unit) {
  val fewer = stringResource(R.string.fewer_beats)
  val more = stringResource(R.string.more_beats)
  Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
    FilledTonalButton(
      onClick = { onChange((value - 1).coerceIn(range)) },
      modifier = Modifier.semantics { contentDescription = fewer },
    ) {
      Text("−")
    }
    Text(value.toString(), style = MaterialTheme.typography.headlineMedium)
    FilledTonalButton(
      onClick = { onChange((value + 1).coerceIn(range)) },
      modifier = Modifier.semantics { contentDescription = more },
    ) {
      Text("+")
    }
  }
}
