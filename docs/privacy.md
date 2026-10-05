# Privacy

*Metronom, version 0.x. This is the statement the app shows under Settings → About.*

Metronom works entirely on your phone.

- **No internet.** The app does not ask for the INTERNET permission and cannot send anything
  anywhere. There are no accounts, ads, analytics or crash reports.
- **No data collected.** Your songs, setlists and settings are plain markdown files in the app's
  private folder on the phone. Nobody but you has them.
- **What leaves the phone** is only what you choose: a zip you export (to a place you pick in
  Android's file picker) and anything you copy out of it yourself.
- **Permissions it does use:** to play in the background it runs a foreground service of the
  media-playback type, which needs `FOREGROUND_SERVICE` and `FOREGROUND_SERVICE_MEDIA_PLAYBACK`;
  `POST_NOTIFICATIONS` is for the "playing" notification with its Stop button (the app works if
  you deny it).
- **Android's device backup.** If Android's own backup is switched on in your phone's settings,
  Android may copy the app's `Metronom` folder (songs, setlists, settings, nothing else) to your
  Google account when it backs up the phone, and restore it on a new phone. The app takes no part
  in that and cannot see it; you can turn the backup off in the phone's settings.

Questions or problems: open an issue at <https://github.com/martinonstad/Metronome/issues>.
