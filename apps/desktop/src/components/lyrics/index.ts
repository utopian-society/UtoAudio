// This file is part of utoaudio, licensed under AGPL-3.0.
// Derivative work based on AMLL (https://github.com/amll-dev/applemusic-like-lyrics),
// which is also licensed under AGPL-3.0. See LICENSE for full license text.
//
// Lyric rendering is fully delegated to the vendored AMLL core `DomLyricPlayer`
// (bundled from `src/lib/vendor/amll/packages/core/dist/lyric-player.mjs`).
// `LyricPlayer.svelte` is a thin Svelte wrapper that mounts the player and
// forwards props into the player's API.

export { default as LyricPlayer } from "./LyricPlayer.svelte";
export { default as FluidBackground } from "./FluidBackground.svelte";
export type { FluidBackgroundMode } from "./FluidBackground.svelte";

export type {
  AnimationMode,
  KaraokeWord,
  // `LyricLine` is also the Svelte component name exported above; import the
  // type from `./types` (or `../../lib/types/lyrics`) to avoid a clash.
  LyricMetadata,
  LyricPlayerProps,
  LyricRuby,
  LyricSource,
  LyricTheme,
  LyricWord,
  SimpleLyricLine,
} from "../../lib/types/lyrics";
export {
  fromSimpleLyricLines,
  lineKaraokeWords,
  lineText,
  lineTranslations,
  MAX_LRC_TIMESTAMP,
} from "../../lib/types/lyrics";

// Lyric parsing (`parseLyrics` + per-format parsers).
export {
	parseLyrics,
	parseLyricsFull,
	parseLrc,
	parseQrc,
	parseTTML,
	parseTTMLWithMetadata,
	parseYrc,
	stringifyLrc,
	stringifyQrc,
	stringifyYrc,
} from "../../lib/lyric-parser";
export type { LyricFormat } from "../../lib/lyric-parser";

// Theme extraction from album art.
export { extractTheme } from "./color";
