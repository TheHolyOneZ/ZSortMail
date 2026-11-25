# Changelog

## 0.1.1

- Windows: no more terminal window next to the app. The release build was started as a console program, so Windows opened a console with it, and closing that console closed the app. (#1, #3)
- Mail list: Archive, snooze and trash now show when you hover a conversation, or when it has keyboard focus, and the text gets the full width of the row. Settings, Appearance, Row buttons brings them back all the time. (#2)
- Settings, Appearance, Open on: start on People, Mail or Today. People stays the default. (#3)
- ZSortMail can be your mail app. Clicking an email link opens a new message with the address, subject and body filled in. The Windows installer, the deb and the rpm register it, and the macOS bundle declares it. Windows and macOS have not been tried on a real machine yet. (#3)
- Writing to a group: a category page has a Write to everyone button that puts everyone in Bcc, and typing a category's name in To, Cc or Bcc adds all of its people. (#4)
- Bcc: a message you send keeps who it was blind-copied to, and Details shows it. Mail you sent before this version does not have it, because it was never saved. (#6)
- Version 0.1.1.
