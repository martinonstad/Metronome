package no.onstad.metronom

import androidx.compose.runtime.saveable.listSaver

/** The three tabs in the bar at the top of every page. */
enum class Tab {
  /** The metronome, which is also where the current song is edited. */
  Metronome,

  /** All songs and the setlists. */
  Lists,

  /** Settings, and About under it. */
  Settings,
}

/** A list on the Lists tab: every song, or one setlist. */
sealed interface ListId {
  /** A short key, for saving and for remembering the selected row. */
  val key: String

  data object AllSongs : ListId {
    override val key = "*"
  }

  /** [stem] is the setlist's file name without `.md`. */
  data class Setlist(val stem: String) : ListId {
    override val key = "s:$stem"
  }

  companion object {
    fun fromKey(key: String): ListId? =
      when {
        key == AllSongs.key -> AllSongs
        key.startsWith("s:") -> Setlist(key.removePrefix("s:"))
        else -> null
      }
  }
}

/**
 * Where you are. Every tab remembers its own page: the Lists tab keeps the open [list] (and its
 * Edit mode) while you look at another tab, and the Settings tab keeps About open.
 *
 * There are no Back buttons; system Back follows [back], and tapping the tab you are on goes to
 * its first page ([select]).
 */
data class Nav(
  val tab: Tab = Tab.Metronome,
  /** The list open on the Lists tab; `null` is the overview. */
  val list: ListId? = null,
  /** The open list is in Edit mode (setlists only). */
  val editing: Boolean = false,
  /** About is open on the Settings tab. */
  val about: Boolean = false,
  /** The Metronome tab was opened from the open list (its ✎), so Back returns there. */
  val fromList: Boolean = false,
) {
  /** Where system Back goes, or `null` to leave the app. */
  fun back(): Nav? =
    when (tab) {
      Tab.Metronome -> if (fromList && list != null) copy(tab = Tab.Lists, fromList = false) else null
      Tab.Lists ->
        when {
          editing -> copy(editing = false)
          list != null -> copy(list = null)
          else -> copy(tab = Tab.Metronome)
        }
      Tab.Settings -> if (about) copy(about = false) else copy(tab = Tab.Metronome)
    }

  /** A tap on a tab: another tab opens where it was left; the tab you are on goes to its first page. */
  fun select(target: Tab): Nav =
    when {
      target != tab -> copy(tab = target, fromList = false)
      tab == Tab.Lists -> copy(list = null, editing = false)
      tab == Tab.Settings -> copy(about = false)
      else -> copy(fromList = false)
    }

  fun openList(id: ListId, edit: Boolean = false) = copy(tab = Tab.Lists, list = id, editing = edit, fromList = false)

  /** The Metronome tab, opened from the open list: Back returns to it. */
  fun toMetronomeFromList() = copy(tab = Tab.Metronome, editing = false, fromList = list != null)
}

/** Keeps [Nav] across rotation and the process being recreated. */
val NavSaver =
  listSaver<Nav, Any>(
    save = { listOf(it.tab.name, it.list?.key ?: "", it.editing, it.about, it.fromList) },
    restore = {
      Nav(
        tab = Tab.entries.firstOrNull { tab -> tab.name == it[0] } ?: Tab.Metronome,
        list = ListId.fromKey(it[1] as String),
        editing = it[2] as Boolean,
        about = it[3] as Boolean,
        fromList = it[4] as Boolean,
      )
    },
  )
