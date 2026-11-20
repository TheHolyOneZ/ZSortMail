import { describe, expect, it } from 'vitest';
import { shortUrl, tidyText } from './tidy';

const github = `BBfly1 left a comment (TheHolyOneZ/ZDirMap#1)

I was able to open, but it did not install like a regular app.

<img width="1142" height="615" alt="Screenshot" src="https://github.com/user-attachments/assets/731564a0-6db2-4a5a" />

Regards,
bb

--
Reply to this email directly or view it on GitHub:
https://github.com/TheHolyOneZ/ZDirMap/issues/1#issuecomment-1
You are receiving this because you were assigned.

Message ID: <TheHolyOneZ/ZDirMap/issues/1@github.com>`;

describe('tidyText', () => {
  it('separates what was written from notification boilerplate, and loses nothing', () => {
    const t = tidyText(github);
    expect(t.body).toContain('I was able to open');
    expect(t.body).toContain('Regards,\nbb');
    expect(t.body).not.toContain('Reply to this email');
    expect(t.body.endsWith('bb')).toBe(true);
    expect(t.footer).toMatch(/^Reply to this email directly/);
    expect(t.footer).toContain('Message ID:');
    expect(t.footer).toContain('You are receiving this because');
  });

  it('replaces embedded image tags and unwraps angle-bracket links', () => {
    const t = tidyText(
      'See <img src="https://x.example/a.png" alt="a"> and <https://x.example/long/path>.',
    );
    expect(t.images).toBe(1);
    expect(t.body).toBe('See [image] and https://x.example/long/path.');
  });

  it('leaves an ordinary message alone, including a personal signature', () => {
    const t = tidyText('Hi,\n\nLunch on Thursday?\n\n-- \nZo\nZSortMail');
    expect(t.footer).toBeNull();
    expect(t.body).toContain('-- \nZo');
  });

  it('does not fold away a message that is only boilerplate', () => {
    const t = tidyText(
      'You are receiving this because you subscribed.\nUnsubscribe: https://x.example/u',
    );
    expect(t.footer).toBeNull();
    expect(t.body).toContain('You are receiving this');
  });

  it('recognises common footers from other senders', () => {
    for (const marker of [
      'Unsubscribe',
      'Manage your preferences',
      'This email was sent to you@example.com',
      'View this email in your browser',
      "If you don't want to receive these emails, click here",
    ]) {
      const t = tidyText(`The real news is here.\n\n${marker}`);
      expect(t.footer, marker).toBe(marker);
      expect(t.body).toBe('The real news is here.');
    }
  });
});

describe('shortUrl', () => {
  it('shows the site and the start of the path', () => {
    expect(shortUrl('https://example.com/')).toBe('example.com');
    expect(shortUrl('https://www.example.com/a/b')).toBe('example.com/a/b');
    const long = shortUrl(
      'https://github.com/user-attachments/assets/731564a0-6db2-4a5a-9d4a-aaaaaaaaaaaa',
    );
    expect(long.length).toBeLessThanOrEqual(45);
    expect(long.startsWith('github.com/user-attachments')).toBe(true);
    expect(long.endsWith('…')).toBe(true);
    expect(
      shortUrl('not a url that is quite long indeed, longer than the limit allows', 20),
    ).toHaveLength(20);
  });
});
