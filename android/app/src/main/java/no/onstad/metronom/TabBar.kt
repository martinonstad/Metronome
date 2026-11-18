package no.onstad.metronom

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.material3.Tab as TabItem

/**
 * The bar at the top of every page: Metronome, Lists and Settings, each an icon above a short
 * label. The labels stay on one line and grow at most 1.3× with the phone's text size, so the bar
 * fits on a narrow phone at the largest text; a screen reader reads each tab's name and whether
 * it is selected.
 */
@Composable
internal fun TabBar(selected: Tab, onSelect: (Tab) -> Unit) {
  PrimaryTabRow(selectedTabIndex = selected.ordinal) {
    for (tab in Tab.entries) {
      TabItem(
        selected = tab == selected,
        onClick = { onSelect(tab) },
        icon = {
          when (tab) {
            Tab.Metronome -> Icon(painterResource(R.drawable.ic_stat_metronome), contentDescription = null)
            Tab.Lists -> Icon(Icons.AutoMirrored.Filled.List, contentDescription = null)
            Tab.Settings -> Icon(Icons.Filled.Settings, contentDescription = null)
          }
        },
        text = {
          Text(
            stringResource(
              when (tab) {
                Tab.Metronome -> R.string.tab_metronome
                Tab.Lists -> R.string.tab_lists
                Tab.Settings -> R.string.settings
              }
            ),
            fontSize = numeralSize(14),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
          )
        },
      )
    }
  }
}
