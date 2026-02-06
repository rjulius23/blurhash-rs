/**
 * Encode image pixel data into a BlurHash string.
 *
 * @param data - Raw pixel bytes in RGB order (length must be width * height * 3).
 * @param width - Image width in pixels.
 * @param height - Image height in pixels.
 * @param componentsX - Number of horizontal components (1-9, default 4).
 * @param componentsY - Number of vertical components (1-9, default 4).
 * @returns The BlurHash string.
 */
export function encode(
  data: Buffer,
  width: number,
  height: number,
  componentsX?: number,
  componentsY?: number,
): string;

/**
 * Encode from a Uint8Array (for browser/Deno compatibility).
 *
 * @param data - Raw pixel bytes as Uint8Array in RGB order.
 * @param width - Image width in pixels.
 * @param height - Image height in pixels.
 * @param componentsX - Number of horizontal components (1-9, default 4).
 * @param componentsY - Number of vertical components (1-9, default 4).
 * @returns The BlurHash string.
 */
export function encodeFromUint8Array(
  data: Uint8Array,
  width: number,
  height: number,
  componentsX?: number,
  componentsY?: number,
): string;

/**
 * Decode a BlurHash string into raw RGB pixel data.
 *
 * @param blurhash - The BlurHash string to decode.
 * @param width - Desired output width in pixels.
 * @param height - Desired output height in pixels.
 * @param punch - Contrast adjustment factor (default 1.0).
 * @returns A Buffer of length width * height * 3 containing RGB pixel data.
 */
export function decode(
  blurhash: string,
  width: number,
  height: number,
  punch?: number,
): Buffer;

/**
 * Decode a BlurHash string into a Uint8Array (for browser/Deno compatibility).
 *
 * @param blurhash - The BlurHash string to decode.
 * @param width - Desired output width in pixels.
 * @param height - Desired output height in pixels.
 * @param punch - Contrast adjustment factor (default 1.0).
 * @returns A Uint8Array of length width * height * 3 containing RGB pixel data.
 */
export function decodeToUint8Array(
  blurhash: string,
  width: number,
  height: number,
  punch?: number,
): Uint8Array;

/**
 * Result of extracting components from a BlurHash string.
 */
export interface Components {
  componentsX: number;
  componentsY: number;
}

/**
 * Extract the number of X and Y components from a BlurHash string.
 *
 * @param blurhash - The BlurHash string.
 * @returns An object with componentsX and componentsY fields.
 */
export function getComponents(blurhash: string): Components;

/**
 * Convert an sRGB byte value (0-255) to linear RGB (0.0-1.0).
 */
export function srgbToLinear(value: number): number;

/**
 * Convert a linear RGB value (0.0-1.0) to an sRGB byte value (0-255).
 */
export function linearToSrgb(value: number): number;

/**
 * Async version of encode that runs on the libuv thread pool.
 * Does not block the event loop.
 *
 * @param data - Raw pixel bytes in RGB order (length must be width * height * 3).
 * @param width - Image width in pixels.
 * @param height - Image height in pixels.
 * @param componentsX - Number of horizontal components (1-9, default 4).
 * @param componentsY - Number of vertical components (1-9, default 4).
 * @returns A Promise resolving to the BlurHash string.
 */
export function encodeAsync(
  data: Buffer,
  width: number,
  height: number,
  componentsX?: number,
  componentsY?: number,
): Promise<string>;

/**
 * Async version of decode that runs on the libuv thread pool.
 * Does not block the event loop.
 *
 * @param blurhash - The BlurHash string to decode.
 * @param width - Desired output width in pixels.
 * @param height - Desired output height in pixels.
 * @param punch - Contrast adjustment factor (default 1.0).
 * @returns A Promise resolving to a Buffer of RGB pixel data.
 */
export function decodeAsync(
  blurhash: string,
  width: number,
  height: number,
  punch?: number,
): Promise<Buffer>;
