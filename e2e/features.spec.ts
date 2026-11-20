import { expect, type Page, test } from '@playwright/test';

const NOW = '2026-11-18T10:00:00+01:00';
const WIDE = { width: 1360, height: 860 } as const;

function url(hash: string): string {
  return `/?now=${encodeURIComponent(NOW)}#${hash}`;
}

async function ipc<T>(page: Page, cmd: string, data: object): Promise<T> {
  const response = await page.request.post(`/ipc/${cmd}`, { data });
  return (await response.json()) as T;
}

async function personId(page: Page, name: string): Promise<number> {
  const people = await ipc<{ id: number }[]>(page, 'list_people', { filter: 'all', query: name });
  if (!people[0]) throw new Error(`no demo person matches ${name}`);
  return people[0].id;
}

async function conversationId(page: Page, person: number, subject: string): Promise<number> {
  const list = await ipc<{ id: number; subject: string }[]>(page, 'list_conversations', {
    personId: person,
  });
  const found = list.find((c) => c.subject === subject);
  if (!found) throw new Error(`no conversation "${subject}"`);
  return found.id;
}

function watchForErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(`console: ${m.text()}`);
  });
  page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
  return errors;
}

test.use({ viewport: WIDE });

test('Original shows the sender’s formatting in a sandbox, with images held back', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  const dhl = await personId(page, 'DHL');
  const convo = await conversationId(page, dhl, 'Your parcel is on its way');
  await page.goto(url(`/people/${dhl}/c/${convo}`));
  await expect(
    page.getByRole('heading', { level: 2, name: 'Your parcel is on its way' }),
  ).toBeVisible();

  await page.getByRole('radio', { name: 'Original' }).click();
  const frame = page.frameLocator('iframe[title^="Message from"]').first();
  await expect(frame.getByText('Your parcel is on its way')).toBeVisible();
  await expect(frame.getByText('JD014600003394512').first()).toBeVisible();

  const box = await page.locator('iframe[title^="Message from"]').first().boundingBox();
  expect(box?.height ?? 0).toBeGreaterThan(150);

  await expect(page.getByText('1 remote image not loaded.')).toBeVisible();
  await expect(page.getByText('1 tracker blocked.')).toBeVisible();
  await expect(page.locator('iframe[title^="Message from"]').first()).not.toHaveAttribute(
    'src',
    /remote=1/,
  );
  await page.getByRole('button', { name: 'Load images' }).click();
  await expect(page.getByText('Showing 1 remote image.')).toBeVisible();
  await expect(page.locator('iframe[title^="Message from"]').first()).toHaveAttribute(
    'src',
    /remote=1/,
  );

  await page.screenshot({ path: 'test-results/screens/original-view.png' });

  const sandbox = await page
    .locator('iframe[title^="Message from"]')
    .first()
    .getAttribute('sandbox');
  expect(sandbox).toBe('allow-scripts');
  expect(errors.filter((e) => !e.includes('Failed to load resource'))).toEqual([]);
});

test('a link that says one site and goes to another is stopped and explained', async ({ page }) => {
  const payroll = await personId(page, 'Payroll');
  const convo = await conversationId(page, payroll, 'Your payslip for October');
  await page.goto(url(`/people/${payroll}/c/${convo}`));
  await page.getByRole('radio', { name: 'Original' }).click();
  const frame = page.frameLocator('iframe[title^="Message from"]').first();
  await frame.getByRole('link', { name: /hr\.northwind\.example/ }).click();

  const dialog = page.getByRole('dialog', { name: 'Check this link' });
  await expect(dialog).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/link-check.png' });
  await expect(
    dialog.getByText(
      'The link says hr.northwind.example but goes to northwind-hr-login.example.net.',
    ),
  ).toBeVisible();
  await expect(dialog.getByText('https://northwind-hr-login.example.net/verify')).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).toBeHidden();
});

test('an ordinary link opens without a prompt', async ({ page, context }) => {
  const dhl = await personId(page, 'DHL');
  const convo = await conversationId(page, dhl, 'Your parcel is on its way');
  await page.goto(url(`/people/${dhl}/c/${convo}`));
  await page.getByRole('radio', { name: 'Original' }).click();
  const frame = page.frameLocator('iframe[title^="Message from"]').first();
  await context.route('**/tracking.parcel.dhl.example/**', (route) =>
    route.fulfill({ contentType: 'text/html', body: '<title>tracking</title>' }),
  );
  const popup = context.waitForEvent('page');
  await frame.getByRole('link', { name: 'Track your parcel' }).click();
  const opened = await popup;
  expect(opened.url()).toContain('tracking.parcel.dhl.example');
  expect(
    opened.url(),
    'tracking parameters were stripped when the message was stored',
  ).not.toContain('utm_');
  await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('the Files tab lists a person’s attachments and Quick Look shows them', async ({ page }) => {
  const errors = watchForErrors(page);
  const maya = await personId(page, 'Okafor');
  await page.goto(url(`/people/${maya}`));
  await page.getByRole('tab', { name: /^Files/ }).click();
  const files = page.getByRole('list', { name: 'Files' }).getByRole('button');
  await expect(files).toHaveCount(5);
  await expect(files.filter({ hasText: 'Invoice-2291.pdf' })).toBeVisible();

  await files.filter({ hasText: 'tooltip-staging.png' }).click();
  const preview = page.getByRole('dialog', { name: 'Preview' });
  await expect(preview).toBeVisible();
  await expect(preview.getByRole('img', { name: 'tooltip-staging.png' })).toBeVisible();
  await expect(preview.getByText(/1 of 5/)).toBeVisible();
  await page.screenshot({ path: 'test-results/screens/quick-look.png' });
  await page.keyboard.press('ArrowRight');
  await expect(preview.getByText(/2 of 5/)).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(preview).toBeHidden();
  expect(errors).toEqual([]);
});

test('the palette finds a message by the words in it and opens its conversation', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  await page.goto(url('/people'));
  await expect(page.getByRole('listbox', { name: 'People' })).toBeVisible();
  await page.keyboard.press('Control+k');
  const palette = page.getByRole('dialog', { name: 'Search and commands' });
  await expect(palette).toBeVisible();
  await palette.getByRole('combobox').fill('xbox 394');
  const hit = palette.getByRole('option', { name: /Your parcel is on its way/ }).first();
  await expect(hit).toBeVisible();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowUp');
  await hit.click();
  await expect(
    page.getByRole('heading', { level: 2, name: 'Your parcel is on its way' }),
  ).toBeVisible();
  await expect(palette).toBeHidden();
  expect(errors).toEqual([]);
});

test('the palette goes to a screen and accents do not matter', async ({ page }) => {
  await page.goto(url('/people'));
  await expect(page.getByRole('listbox', { name: 'People' })).toBeVisible();
  await page.keyboard.press('/');
  const palette = page.getByRole('dialog', { name: 'Search and commands' });
  await expect(palette).toBeVisible();
  await palette.getByRole('combobox').fill('settings');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/settings/);

  await page.goto(url('/people'));
  await expect(page.getByRole('listbox', { name: 'People' })).toBeVisible();
  await page.keyboard.press('Control+k');
  await palette.getByRole('combobox').fill('amelie');
  await expect(palette.getByRole('option', { name: /^Amélie Dubois/ })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(palette).toBeHidden();
});

test('notes save by themselves, a pinned person moves up, and an edit shows everywhere', async ({
  page,
}) => {
  const errors = watchForErrors(page);
  const rasmus = await personId(page, 'Lindqvist');
  await page.goto(url(`/people/${rasmus}`));

  await page.getByRole('tab', { name: /^Files/ }).click();
  await page.screenshot({ path: 'test-results/screens/files-tab.png' });
  await page.getByRole('tab', { name: 'Notes' }).click();
  await page.getByLabel('Notes about this person').fill('Files VAT on the 27th.');
  await expect(page.getByText('Saved')).toBeVisible();
  const detail = await ipc<{ notes: string }>(page, 'get_person', { id: rasmus });
  expect(detail.notes).toBe('Files VAT on the 27th.');

  await page.getByRole('button', { name: 'Pin to the top' }).click();
  await expect(page.getByRole('button', { name: 'Unpin' })).toBeVisible();
  await expect(page.getByRole('option').nth(2)).toContainText('Rasmus');

  await page.getByRole('button', { name: 'Edit person' }).click();
  const dialog = page.getByRole('dialog', { name: /Edit Rasmus/ });
  await dialog.getByLabel('Role').fill('Tax adviser');
  await dialog.getByRole('button', { name: 'Save' }).click();
  await expect(dialog).toBeHidden();
  await expect(page.getByText('Tax adviser, Lindqvist & Moe')).toBeVisible();

  await ipc(page, 'set_person_pinned', { id: rasmus, pinned: false });
  await ipc(page, 'edit_person', {
    id: rasmus,
    edit: {
      name: 'Rasmus Lindqvist',
      title: 'Accountant',
      organization: 'Lindqvist & Moe',
      description: 'Files VAT on the 27th. Do not send invoices after the 25th.',
      ink: 4,
    },
  });
  await ipc(page, 'set_person_notes', { id: rasmus, notes: '' });
  expect(errors).toEqual([]);
});

test('opening a conversation marks it read after a moment', async ({ page }) => {
  const dad = await personId(page, 'Dad');
  const convo = await conversationId(page, dad, 'dinner sat?');
  const before = await ipc<{ unread: number }>(page, 'get_conversation', {
    id: convo,
    viewerPersonId: dad,
  });
  test.skip(before === null, 'demo not loaded');
  await page.goto(url(`/people/${dad}/c/${convo}`));
  await expect
    .poll(
      async () =>
        (
          await ipc<{ summary: { unread: number } }>(page, 'get_conversation', {
            id: convo,
            viewerPersonId: dad,
          })
        ).summary.unread,
      {
        timeout: 6000,
      },
    )
    .toBe(0);
  await ipc(page, 'mark_conversation_read', { id: convo, read: false });
});

test('a PDF is drawn by the app itself, with pages and zoom', async ({ page }) => {
  const errors = watchForErrors(page);
  const maya = await personId(page, 'Okafor');
  await page.goto(url(`/people/${maya}`));
  await page.getByRole('tab', { name: /^Files/ }).click();
  await page
    .getByRole('list', { name: 'Files' })
    .getByRole('button')
    .filter({ hasText: 'Invoice-2291.pdf' })
    .click();
  const preview = page.getByRole('dialog', { name: 'Preview' });
  await expect(preview.getByText(/^Page 1 of \d+, 100%$/)).toBeVisible({ timeout: 15_000 });
  const canvas = preview.getByRole('img', { name: 'Page 1' });
  await expect(canvas).toBeVisible();
  await expect
    .poll(async () => canvas.evaluate((c: HTMLCanvasElement) => c.width), { timeout: 10_000 })
    .toBeGreaterThan(100);
  const varied = await canvas.evaluate((c: HTMLCanvasElement) => {
    const d = c.getContext('2d')?.getImageData(0, 0, c.width, c.height).data;
    if (!d) return false;
    for (let i = 4; i < d.length; i += 4)
      if (d[i] !== d[0] || d[i + 1] !== d[1] || d[i + 2] !== d[2]) return true;
    return false;
  });
  expect(varied).toBe(true);
  await page.screenshot({ path: 'test-results/screens/pdf.png' });
  await preview.getByRole('button', { name: 'Zoom in' }).click();
  await expect(preview.getByText(/125%/)).toBeVisible();
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
});
