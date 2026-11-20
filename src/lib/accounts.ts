import type { AccountInfo } from './bindings';

export function accountLabel(account: AccountInfo): string {
  return account.provider === 'demo' ? (account.display_name ?? 'Demo mailbox') : account.email;
}
