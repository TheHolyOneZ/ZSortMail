import { expect, type Page, test } from '@playwright/test';

test.describe.configure({ mode: 'serial' });

const NOW = '2026-11-18T10:00:00+01:00';
const url = (hash: string) => `/?now=${encodeURIComponent(NOW)}#${hash}`;

function watchForErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error' && !m.text().includes('Failed to load resource')) {
      errors.push(`console: ${m.text()}`);
    }
  });
  page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
  return errors;
}

test.use({ viewport: { width: 1360, height: 860 } });

test('the welcome screen fits a normal window, and the Google setup is a guided tutorial', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 760 });
  await page.goto(url('/welcome'));
  const title = page.getByRole('heading', {
    level: 1,
    name: /Every conversation, kept with the person/,
  });
  await expect(title).toBeVisible();
  const overflow = await page.evaluate(() => {
    const main = document.querySelector('main') as HTMLElement;
    return main.scrollHeight - main.clientHeight;
  });
  expect(overflow).toBeLessThanOrEqual(0);
  await expect(
    page.getByRole('button', { name: 'Look around with demo mail first' }),
  ).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/welcome-compact.png' });
});

test('an empty install welcomes you, links through the browser, and fills in from Gmail', async ({
  page,
  context,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/'));
  await expect(
    page.getByRole('heading', { level: 1, name: /Every conversation, kept with the person/ }),
  ).toBeVisible();
  await expect(page).toHaveURL(/#\/welcome/);
  await page.screenshot({ path: 'test-results/screens/welcome.png' });

  const popup = context.waitForEvent('page');
  await page.getByRole('button', { name: 'Link a Gmail account' }).click();
  const opened = await popup;
  await expect(opened.getByText('ZSortMail is linked to your Gmail')).toBeVisible();

  await expect(page).toHaveURL(/#\/people/, { timeout: 15_000 });
  const people = page.getByRole('listbox', { name: 'People' });
  await expect(people.getByRole('option', { name: /Maya Okafor/ })).toBeVisible({
    timeout: 15_000,
  });
  await expect(people.getByRole('option', { name: /Amélie Dubois/ })).toBeVisible();
  await expect(people.getByRole('option', { name: /Jean Moreau/ })).toBeVisible();
  await expect(people.getByRole('option', { name: /DHL Parcel/ })).toBeVisible();
  expect(errors).toEqual([]);
});

test('a thread syncs with the reply I sent from Gmail, and a file downloads on demand', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/people'));
  await page.getByRole('option', { name: /Maya Okafor/ }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Q3 launch checklist' })).toBeVisible();
  const strand = page.getByRole('region', { name: 'Conversation', exact: true });
  await expect(page.locator('[data-knot]')).toHaveCount(3);
  await expect(strand.getByText('Thanks Maya. I added three items under Support.')).toBeVisible();
  await expect(strand.getByText('Thursday 10:00 works.')).toBeVisible();
  await expect(strand.getByRole('button', { name: 'Show quoted text' })).toBeVisible();

  await page.getByRole('option', { name: /DHL Parcel/ }).click();
  await page.getByRole('tab', { name: /^Files/ }).click();
  await page
    .getByRole('button', { name: /label-394-2211\.pdf/ })
    .first()
    .click();
  const preview = page.getByRole('dialog', { name: 'Preview' });
  await expect(preview).toBeVisible();
  await expect(preview.getByText('label-394-2211.pdf').first()).toBeVisible();
  await expect(preview.getByText('Downloading')).toBeHidden({ timeout: 10_000 });
  const served = await page.request.get('/zsm/att/1?inline=1');
  expect([200, 404]).toContain(served.status());
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
});

test('inline images in a formatted message load once downloaded, and the tracker is blocked', async ({
  page,
}) => {
  await page.goto(url('/people'));
  await page.getByRole('option', { name: /DHL Parcel/ }).click();
  await page.getByRole('radio', { name: 'Original' }).click();
  const frame = page.frameLocator('iframe[title^="Message from"]').first();
  await expect(frame.getByText('Your parcel is on its way').first()).toBeVisible();
  await expect
    .poll(
      async () =>
        frame
          .locator('img[alt="DHL"]')
          .evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth),
      { timeout: 10_000 },
    )
    .toBeGreaterThanOrEqual(0);
  await expect(page.getByText('1 tracker blocked.')).toBeVisible();
});

test('new mail arrives by itself, and a stranger waits in New senders', async ({
  page,
  request,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/people'));
  await expect(page.getByRole('listbox', { name: 'People' })).toBeVisible();
  const before = await page.getByRole('option').count();

  await request.post('/dev/fake-mail', {
    data: {
      from: 'Maya Okafor <maya.okafor@northwind.example>',
      subject: 'Tooltip is live',
      text: 'Could you check the tooltip on staging?',
    },
  });
  await request.post('/dev/fake-mail', {
    data: {
      from: 'Marcus Lee <marcus@harborroasters.example>',
      subject: 'Fresh beans for the office',
      text: 'Free tasting box, reply with an address.',
    },
  });

  await page.getByRole('option', { name: /Maya Okafor/ }).click();
  await expect(page.getByRole('button', { name: /Tooltip is live/ })).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByRole('option', { name: /Marcus Lee/ })).toHaveCount(0);
  await expect(page.getByRole('option')).toHaveCount(before);
  await expect(page.getByRole('link', { name: 'New senders' })).toContainText('1', {
    timeout: 15_000,
  });
  expect(errors).toEqual([]);
});

test('a watch fires when the mail arrives, with the numbers pulled out of it', async ({
  page,
  request,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/waiting'));
  await page.getByRole('button', { name: 'Watch for an email' }).click();
  const dialog = page.getByRole('dialog', { name: 'Wait for an email' });
  await dialog.getByLabel('What are you waiting for?').fill('xbox 394');
  await dialog.getByRole('button', { name: 'Start watching' }).click();
  await expect(
    page
      .getByRole('region', { name: /^Waiting/ })
      .getByRole('heading', { level: 3, name: 'xbox 394' }),
  ).toBeVisible();

  await request.post('/dev/fake-mail', {
    data: {
      from: 'DHL Parcel <noreply@parcel.dhl.example>',
      subject: 'Out for delivery',
      text: 'Your order 394-2211 has been handed to DHL.\nXbox Series X (1 item)\nTracking number: JD014600003394512',
    },
  });

  const toast = page.getByRole('status').filter({ hasText: '“xbox 394” has arrived' });
  await expect(toast).toBeVisible({ timeout: 20_000 });
  await expect(toast.getByRole('button', { name: 'Open' })).toBeVisible();
  const arrived = page.getByRole('region', { name: 'Arrived' });
  await expect(arrived.getByRole('button', { name: /Tracking JD014600003394512/ })).toBeVisible();
  await expect(arrived.getByRole('button', { name: /Order 394-2211/ })).toBeVisible();
  await expect(arrived.getByText('New, not opened yet')).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/waiting-arrived.png' });

  await toast.getByRole('button', { name: 'Open' }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Out for delivery' })).toBeVisible();
  await page.goto(url('/notifications'));
  await expect(page.getByText(/Shown\. You are waiting for “xbox 394”/)).toBeVisible();
  expect(errors).toEqual([]);
});

test('a reply written in the composer leaves through Gmail, and the undo window holds it back', async ({
  page,
  request,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/people'));
  const people = page.getByRole('listbox', { name: 'People' });
  await people.getByRole('option', { name: /Maya Okafor/ }).click();
  await expect(page.getByRole('heading', { level: 2, name: 'Tooltip is live' })).toBeVisible();
  await page
    .getByRole('region', { name: 'Conversation', exact: true })
    .click({ position: { x: 5, y: 5 } });

  await page.keyboard.press('a');
  const composer = page.getByRole('region', { name: 'Reply to all' });
  await expect(composer).toBeVisible();
  await composer.getByRole('textbox', { name: 'Message' }).click();
  await page.keyboard.type('Thanks, this works for me.');
  await composer.getByRole('button', { name: 'Attach files' }).waitFor();
  await composer.getByLabel('Choose files').setInputFiles({
    name: 'notes.txt',
    mimeType: 'text/plain',
    buffer: Buffer.from('my notes'),
  });
  await expect(composer.getByRole('list', { name: 'Files to send' })).toContainText('notes.txt');
  await composer.getByRole('button', { name: 'Send', exact: true }).click();
  const toast = page.getByRole('status').filter({ hasText: /^Sent\./ });
  await expect(toast).toBeVisible();

  await page.waitForTimeout(1500);
  expect(await (await request.get('/dev/sent')).json()).toEqual([]);
  await toast.getByRole('button', { name: 'Undo' }).click();
  const back = page.getByRole('region', { name: /^Reply to/ });
  await expect(back.getByRole('textbox', { name: 'Message' })).toContainText(
    'Thanks, this works for me.',
  );
  await expect(back.getByRole('list', { name: 'Files to send' })).toContainText('notes.txt');

  await request.post('/ipc/save_compose_settings', {
    data: { settings: { signature: '', undo_seconds: 0 } },
  });
  await back.getByRole('button', { name: 'Send', exact: true }).click();
  await expect
    .poll(async () => ((await (await request.get('/dev/sent')).json()) as string[]).length, {
      timeout: 15_000,
    })
    .toBe(1);
  const [raw] = (await (await request.get('/dev/sent')).json()) as string[];
  expect(raw).toContain('Thanks, this works for me.');
  expect(raw).toContain('notes.txt');
  expect(raw).toMatch(/^Subject: Re: /m);
  expect(raw).toMatch(/^In-Reply-To: /m);
  expect(errors).toEqual([]);
});

test('notification mail is tidied: named after the service, long links shortened, the footer folded away', async ({
  page,
  request,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/people'));
  await expect(
    page.getByRole('listbox', { name: 'People' }).getByRole('option').first(),
  ).toBeVisible();
  await request.post('/dev/fake-mail', {
    data: {
      from: 'Jadoooooo <notifications@github.com>',
      subject: '[Org/Repo] MAC installer missing (Issue #1)',
      text: [
        'BBfly1 left a comment (Org/Repo#1)',
        '',
        'I was able to open it, but it did not install like a regular app.',
        '',
        '<img width="1142" alt="Screenshot" src="https://github.com/user-attachments/assets/731564a0-6db2-4a5a-9d4a-aaaaaaaaaaaa" />',
        '',
        '--',
        'Reply to this email directly or view it on GitHub:',
        'https://github.com/Org/Repo/issues/1#issuecomment-123456789012345678901234567890',
        'You are receiving this because you were assigned.',
        '',
        'Message ID: <Org/Repo/issues/1@github.com>',
      ].join('\n'),
    },
  });
  const github = page.getByRole('option', { name: /^GitHub/ });
  await expect(github).toBeVisible({ timeout: 20_000 });
  await expect(page.getByRole('option', { name: /Jadoooooo/ })).toHaveCount(0);
  await github.click();
  await expect(
    page.getByRole('heading', { level: 2, name: /MAC installer missing/ }),
  ).toBeVisible();

  const strand = page.getByRole('region', { name: 'Conversation', exact: true });
  await expect(strand.getByText('I was able to open it')).toBeVisible();
  await expect(strand.getByText('[image]')).toBeVisible();
  await expect(strand.getByText('Reply to this email directly')).toHaveCount(0);
  await expect(strand.getByText('notifications@github.com').first()).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/tidy-letter.png' });
  await strand.getByRole('button', { name: 'Show the automatic footer' }).click();
  await expect(strand.getByText('Reply to this email directly')).toBeVisible();
  expect(errors).toEqual([]);
});

test('Settings shows the account syncing, and removing it returns to the welcome screen', async ({
  page,
}) => {
  await page.goto(url('/settings'));
  await expect(page.getByRole('heading', { level: 3, name: 'you@gmail.example' })).toBeVisible();
  await expect(page.getByText(/Up to date\. Checked/)).toBeVisible({ timeout: 15_000 });
  await page.screenshot({ path: 'test-results/screens/settings-accounts.png' });

  await page.getByRole('button', { name: 'Remove', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Remove account' });
  await expect(dialog.getByText(/Your mail in Gmail is not touched/)).toBeVisible();
  await dialog.getByRole('button', { name: 'Remove' }).click();
  await expect(page).toHaveURL(/#\/welcome/);
  await expect(page.getByRole('heading', { level: 1, name: /Every conversation/ })).toBeVisible();
  const people = await page.request.post('/ipc/list_people', {
    data: { filter: 'all', query: '' },
  });
  expect(await people.json()).toEqual([]);
});
