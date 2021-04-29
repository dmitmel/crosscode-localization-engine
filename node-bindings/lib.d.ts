export const FFI_BRIDGE_VERSION: number;
export const VERSION: string;
export const PROTOCOL_VERSION: number;
export function init_logging(): void;
export class Backend {
  send_message(text: Buffer | string): void;
  recv_message(callback: (err: Error | null, message: Buffer) => void): void;
  close(): void;
  is_closed(): boolean;
}
