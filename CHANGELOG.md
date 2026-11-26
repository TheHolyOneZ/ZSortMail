# Changelog

## 0.1.3

Labels and Settings, from the third round of issues.

- Mail, Labels: hide the labels you do not use. Hover a label and press the eye, or use Settings, Sidebar, Labels in Mail. Hiding only affects this sidebar, not Gmail. Gmail's own labels, like YELLOW_STAR, CATEGORY_* and [Gmail]/..., no longer show in the list. (#11)
- Settings: section headings are larger and have a rule above them, so they stand apart from the setting titles under them. The group names in Keyboard are bolder. (#10)
- Version 0.1.3.

## 0.1.2

Notifications, from the second round of issues.

- Settings, Notifications, Every new message from people: notify when someone you already write to sends something new. Off by default. New senders, newsletters, receipts and automated mail stay quiet, and quiet hours, Focus mode, muting and hiding still apply. (#7)
- Settings, Notifications, Send a test: shows a notification at once and records it in the log, and shows the error if the system refuses. (#7)
- The notification log now records every new message from a person, notified or not, and says why a message did not notify. It has Everything, Notified and Not notified filters and keeps the newest 500. (#9)
- The notification settings say what each switch does: Notify me, and Mail Gmail marks as important, which adds to what notifies. (#8)
- Version 0.1.2.

## 0.1.1

- Windows: no more terminal window next to the app. The release build was started as a console program, so Windows opened a console with it, and closing that console closed the app. (#1, #3)
- Mail list: Archive, snooze and trash now show when you hover a conversation, or when it has keyboard focus, and the text gets the full width of the row. Settings, Appearance, Row buttons brings them back all the time. (#2)
- Settings, Appearance, Open on: start on People, Mail or Today. People stays the default. (#3)
- ZSortMail can be your mail app. Clicking an email link opens a new message with the address, subject and body filled in. The Windows installer, the deb and the rpm register it, and the macOS bundle declares it. Windows and macOS have not been tried on a real machine yet. (#3)
- Writing to a group: a category page has a Write to everyone button that puts everyone in Bcc, and typing a category's name in To, Cc or Bcc adds all of its people. (#4)
- Bcc: a message you send keeps who it was blind-copied to, and Details shows it. Mail you sent before this version does not have it, because it was never saved. (#6)
- Version 0.1.1.
