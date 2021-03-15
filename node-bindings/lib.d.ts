export const FFI_BRIDGE_VERSION: number;
export const VERSION: string;
export const PROTOCOL_VERSION: number;
export function init_logging(): void;
export class Backend {
  send_message(text: string): void;
  recv_message(callback: (err: Error | null, message: string) => void): void;
}
