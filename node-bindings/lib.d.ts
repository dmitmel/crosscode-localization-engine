export const FFI_BRIDGE_VERSION: number;
export const VERSION: string;
export const PROTOCOL_VERSION: number;
export function init_logging(): void;
export class Backend {
  send_message(value: unknown): void;
  recv_message(callback: (err: Error | null, value: unknown) => void): void;
  recv_message_sync(): unknown;
  close(): void;
  is_closed(): boolean;
}
