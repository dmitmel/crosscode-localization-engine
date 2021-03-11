export const VERSION: string;
export function init_logging(): void;
export class Backend {
  send_message(text: string): void;
  recv_message(): string;
}
