# Connecting Gmail

Google only lets an app read your Gmail through a "client" that the app's owner registers with them. We don't have one that Google has reviewed yet, so you make your own. Ten minutes, free, and the two values you get stay on your computer. ZSortMail also walks you through this the first time you press Link a Gmail account.

1. Make a project: https://console.cloud.google.com/projectcreate (name it anything, ZSortMail is fine)
2. Switch on the Gmail API: https://console.cloud.google.com/apis/library/gmail.googleapis.com
3. Set up the consent screen at https://console.cloud.google.com/auth/overview. Choose External, and use your own address wherever it asks for an email.
4. Add yourself as a test user at https://console.cloud.google.com/auth/audience. Skip this and Google says "access_denied".
5. Add the scopes at https://console.cloud.google.com/auth/scopes: openid, email, profile and https://www.googleapis.com/auth/gmail.modify
6. Create a client at https://console.cloud.google.com/auth/clients. Pick "Desktop app" as the type, anything else breaks later.
7. Paste the Client ID and secret into ZSortMail. Pasting the JSON file Google lets you download fills in both.

When your browser opens Google it will say the app isn't verified. That's normal, it's your own: Advanced, then Go to ZSortMail. Leave the Gmail box ticked on the permissions screen, because Google lets you untick it and then nothing syncs.

While the project is in Testing, Google logs you out after a week and ZSortMail shows a Link again button. Setting the project to In production should stop that, though I haven't tried it.

If it fails: "access_denied" means step 4, "redirect_uri_mismatch" means step 6.
