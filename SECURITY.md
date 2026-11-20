# Security

## Reporting a problem

Please report vulnerabilities privately, not in a public issue. Use the repository's private vulnerability reporting (the "Security" tab, "Report a vulnerability"). Include what you did, what you expected and what happened. You will get an answer within a few days.

## What ZSortMail protects, and how

ZSortMail reads and sends mail on your behalf, so the things worth defending are your mail, your Google credentials and your contacts.

- **No server.** Mail is synced from your provider straight into a database on your machine. There is no ZSortMail backend that could leak it, and no telemetry.
- **Credentials.** The OAuth refresh token is stored in the operating system's keychain, never in the database or in a log.
- **Hostile mail.** HTML mail is untrusted input. It is sanitized in Rust when it arrives, then rendered in an iframe with no script permission, from the `zsm://` origin, under a Content-Security-Policy of `default-src 'none'` with images and fonts limited to the app's own protocol. That origin is granted no capability, so a message cannot call into the app. Remote images are blocked until you allow them, which also defeats tracking pixels.
- **Layered, and tested as layers.** The sandbox and the CSP are meant to hold independently of each other and of the sanitizer. The development spikes page runs sixteen attack pages against the sandbox alone, the CSP alone, and a control with neither, and checks that none reaches a local listener. With the sandbox and policy that ship, none of them got through.
- **Least privilege.** Each window has its own capability file (`src-tauri/capabilities/`). There are no remote scripts and no `eval`.
- **Data safety.** The app never permanently deletes mail on the server. Hiding is local.

## Known limits

- The database is not encrypted at rest yet; it relies on your disk encryption. An opt-in encrypted database is planned.
- Alerts and recurring emails need the app, or its tray icon, to be running. Gmail offers no push without a server.
- Public distribution of an app that reads Gmail needs Google's OAuth verification for the restricted `gmail.modify` scope. Until then ZSortMail is for people who create their own Google Cloud OAuth client (the README explains how).
