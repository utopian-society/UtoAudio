// This file is part of utoaudio, licensed under AGPL-3.0.
//
// `file-browser` — file system scanning helpers used by the Library page
// and Settings → Library scan-root panel.
//
// Implementation: thin wrappers around the `scan_directory` and
// `scan_library` Tauri commands registered in `crates/audio-ffi`. Errors
// from the Rust side (permission denied, missing directory, …) surface
// to the caller via the rejected promise — we do NOT silently fall back
// to a demo tree (the prompt-5 stub is retired).

import { invoke } from "@tauri-apps/api/core";

/** A single entry returned by `scanDirectory` / `scanLibrary`. */
export interface FileEntry {
  /** Base name (no directory component). */
  name: string;
  /** Absolute filesystem path. */
  path: string;
  /** Whether this entry is a directory. */
  isDirectory: boolean;
  /** File size in bytes (undefined for directories). */
  size?: number;
  /** Last-modified time in milliseconds since epoch (undefined if unknown). */
  modified?: number;
}

/**
 * Convenience alias kept for source-compatibility with the prompt-5 API
 * surface — the playlist's "Add files" feature uses it as the structural
 * type of an audio file returned by the scanner.
 */
export type AudioEntry = FileEntry;

/** Audio file extensions the Library page recognises. */
export const AUDIO_EXTENSIONS: readonly string[] = [
  ".flac",
  ".wav",
  ".mp3",
  ".opus",
  ".ogg",
  ".aac",
  ".m4a",
  ".wv",
  ".dsf",
  ".dff",
  ".aiff",
  ".ape",
  ".wma",
];

/**
 * Lower-case helper: is `name` a recognised audio file?
 *
 * Exported so the Library page can re-use the same list when it needs to
 * filter ad-hoc paths (e.g. when adding files to a playlist).
 */
export function isAudioFile(name: string): boolean {
  const lower = name.toLowerCase();
  return AUDIO_EXTENSIONS.some((ext) => lower.endsWith(ext));
}

/**
 * Scan a directory and return its immediate children.
 *
 * Thin wrapper around the Rust `scan_directory` command. Errors from
 * permission denial, missing paths, etc. propagate as a rejected promise.
 *
 * @param path         Directory path to scan. Pass an empty string to
 *                     scan the configured scan roots index — the Rust
 *                     command is a no-op for invalid input.
 * @param _extensions  Extension filter; preserved on the signature for
 *                     prompt-5 call-site stability but unused by the
 *                     single-directory listing.
 */
export async function scanDirectory(
  path: string,
  _extensions: readonly string[] = AUDIO_EXTENSIONS,
): Promise<FileEntry[]> {
  return await invoke<FileEntry[]>("scan_directory", { path });
}

/**
 * Walk every root in `roots` up to a bounded depth and return all
 * audio-file entries (case-insensitive extension match, with or without
 * a leading `.`).
 *
 * @param roots       Directories to walk.
 * @param extensions  Extension filter (defaults to all known audio
 *                    formats when omitted). `'.flac'`, `'flac'` and
 *                    `'FLAC'` are all accepted.
 */
export async function listAudioFiles(
  path: string,
  extensions: readonly string[] = AUDIO_EXTENSIONS,
): Promise<FileEntry[]> {
  return await invoke<FileEntry[]>("scan_library", {
    roots: [path],
    extensions: [...extensions],
  });
}

/**
 * Library-wide recursive scan across multiple roots.
 *
 * Used by the Settings → Library "Rescan now" button: the command emits
 * a `library:rescanned` event after this resolves so the Library page can
 * refresh its view automatically.
 *
 * @param roots       Directories to walk.
 * @param extensions  Extension filter (see `listAudioFiles`).
 */
export async function scanLibrary(
  roots: string[],
  extensions: readonly string[] = AUDIO_EXTENSIONS,
): Promise<FileEntry[]> {
  return await invoke<FileEntry[]>("scan_library", {
    roots,
    extensions: [...extensions],
  });
}
