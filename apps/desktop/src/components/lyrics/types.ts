// This file is part of utoaudio, licensed under AGPL-3.0.
// Derivative work based on AMLL (https://github.com/amll-dev/applemusic-like-lyrics),
// which is also licensed under AGPL-3.0. See LICENSE for full license text.

/**
 * Component-local types entry point (per the project's deliverable layout).
 *
 * The canonical definitions live in `../../lib/types/lyrics.ts` so the parsers
 * and components share one source of truth; this file re-exports them to give
 * the `components/lyrics/` package the `types.ts` the layout expects and to
 * keep imports short from sibling component files.
 */
export type {
	AnimationMode,
	KaraokeWord,
	LyricLine,
	LyricMetadata,
	LyricPlayerProps,
	LyricRuby,
	LyricSource,
	LyricTheme,
	LyricWord,
	SimpleLyricLine,
} from '../../lib/types/lyrics';
export {
	fromSimpleLyricLines,
	lineKaraokeWords,
	lineText,
	lineTranslations,
	MAX_LRC_TIMESTAMP,
} from '../../lib/types/lyrics';

/** Background rendering modes supported by {@link FluidBackground}. */
export type { FluidBackgroundMode } from './FluidBackground.svelte';
