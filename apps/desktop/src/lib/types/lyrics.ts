// This file is part of utoaudio, licensed under AGPL-3.0.
// Derivative work based on AMLL (https://github.com/amll-dev/applemusic-like-lyrics),
// which is also licensed under AGPL-3.0. See LICENSE for full license text.

/**
 * Type definitions for the utoaudio lyric subsystem.
 *
 * These types are the public contract of the Svelte 5 port of the AMLL
 * (Apple Music Like Lyrics) lyric player. They stay close to the upstream
 * AMLL data model (`packages/core/src/interfaces.ts` and
 * `packages/lyric/src/types.ts`) so parsed lyrics carry everything the
 * renderer needs (per-word timing, background/duet flags, translations,
 * romanizations), while also exposing a flat convenience shape for simpler
 * callers.
 *
 * All AMLL upstream references are API/behaviour references only — the
 * implementation in this project is hand-written in Svelte 5 / TypeScript.
 */

/**
 * A single timed word inside a lyric line.
 *
 * For syllable-timed formats (TTML / YRC / QRC) each word carries its own
 * `[startTime, endTime]` window; for line-timed formats (LRC) every line is
 * stored as a single word whose window equals the line's window.
 */
export interface LyricWord {
	/** The word's start time, in milliseconds. */
	startTime: number;
	/** The word's end time, in milliseconds. */
	endTime: number;
	/** The word's text content. */
	word: string;
	/** Optional phonetic transliteration (e.g. pinyin / romaji). */
	romanWord?: string;
	/** Whether the word is flagged as containing obscene content. */
	obscene?: boolean;
	/** Optional ruby annotation segments for the word. */
	ruby?: LyricRuby[];
}

/** A ruby (annotation) segment attached to a word. */
export interface LyricRuby {
	startTime: number;
	endTime: number;
	text: string;
}

/**
 * Convenience alias used by the simpler flat lyric contract: a single timed
 * karaoke token.
 */
export interface KaraokeWord {
	startTime: number;
	endTime: number;
	text: string;
}

/**
 * A single line of lyrics.
 *
 * Mirrors the upstream AMLL `LyricLine`: `words` is the canonical
 * representation. Use {@link lineText}, {@link lineTranslations} and
 * {@link lineKaraokeWords} for the flat projections (`text`, `translations`,
 * `karaokeWords`) the rest of the prompt contract refers to.
 */
export interface LyricLine {
	/** The words that make up the line. */
	words: LyricWord[];
	/** Translated lyric text shown beneath the main line. */
	translatedLyric: string;
	/** Phonetic / romanized lyric text shown beneath the translation. */
	romanLyric: string;
	/** The line's start time, in milliseconds. Not always equal to the first word's start. */
	startTime: number;
	/**
	 * The line's end time, in milliseconds.
	 *
	 * The last line keeps a large sentinel end time (the value exported as
	 * {@link MAX_LRC_TIMESTAMP}); parsers compute it from the next line's
	 * start time when possible.
	 */
	endTime: number;
	/** Whether this is a background-vocal line (attached to the previous main line). */
	isBG: boolean;
	/** Whether this is a duet line (rendered right-aligned). */
	isDuet: boolean;
}

/**
 * Metadata returned alongside parsed lyrics for formats that carry extra
 * head information (notably TTML).
 */
export interface LyricMetadata {
	/** Raw metadata entries as `[key, values[]]` pairs. */
	entries: [string, string[]][];
}

/**
 * The result of parsing a lyric document.
 */
export interface LyricSource {
	/** Parsed lyric lines, ordered by start time. */
	lines: LyricLine[];
	/** Optional document metadata. */
	metadata?: LyricMetadata;
}

/**
 * Animation rendering modes for the lyric player.
 *
 * - `default` — Apple Music–style spring scroll + mask-sweep karaoke highlight.
 * - `karaoke` — force the per-word mask-sweep highlight even for whole-line lyrics.
 * - `scrolling` — classic continuous scroll (no active-line emphasis spring).
 * - `static` — no scroll animation; lines render at fixed positions.
 */
export type AnimationMode = 'default' | 'karaoke' | 'scrolling' | 'static';

/**
 * Themed colours derived from album art, consumed by the lyric + background
 * components via CSS custom properties (`--amll-lp-color`, …).
 */
export interface LyricTheme {
	/** Primary foreground/text colour for the lyric layer. */
	color: string;
	/** A palette of dominant colours, used by the fluid background. */
	palette: string[];
	/** Whether the theme was derived from a light album (affects contrast). */
	light: boolean;
}

/**
 * The props accepted by the Svelte 5 `LyricPlayer` component.
 *
 * This is the union of the upstream AMLL React `LyricPlayerProps` API surface
 * and the extra presentation knobs specified by this project.
 */
export interface LyricPlayerProps {
	/** The lyrics to render. Internally cloned — callers may keep mutating their copy. */
	lyrics: LyricLine[];
	/** Current playback time in **milliseconds** (integers are most accurate). */
	currentTime: number;
	/** Whether the player is actively playing (drives interlude / float animations). */
	playing?: boolean;
	/** Fired when the active line index changes. */
	onLineChange?: (lineIndex: number) => void;
	/** Animation mode (see {@link AnimationMode}). Defaults to `default`. */
	animationMode?: AnimationMode;
	/** Optional themed colours derived from album art. */
	theme?: LyricTheme;
	/** Render height in CSS pixels (`undefined` → fill the parent). */
	height?: number;
	/** Render width in CSS pixels (`undefined` → fill the parent). */
	width?: number;
	/** Vertical alignment of the active line, `[0..1]` of the player height. Default `0.35`. */
	alignPosition?: number;
	/** Which part of the active line aligns to {@link LyricPlayerProps.alignPosition}. Default `center`. */
	alignAnchor?: 'top' | 'bottom' | 'center';
	/** Enable spring-physics scrolling (default `true`). */
	enableSpring?: boolean;
	/** Enable per-line blur of inactive lines (default `true`). */
	enableBlur?: boolean;
	/** Enable per-line scale-down of inactive lines (default `true`). */
	enableScale?: boolean;
	/** Hide lines that have already been sung (default `false`). */
	hidePassedLines?: boolean;
	/** Width of the karaoke fade sweep, in multiples of the font size. Default `0.5`. Must be `> 0`. */
	wordFadeWidth?: number;
	/** Whether the player is currently seeking (disables the staggered delay wave). */
	isSeeking?: boolean;
	/** Whether to render the fluid album-art background. Default `false`. */
	enableFluidBackground?: boolean;
}

/**
 * A flat lyric line variant used when callers have only simple line-timed data,
 * matching the minimal shape required by parts of the prompt contract.
 */
export interface SimpleLyricLine {
	startTime: number;
	endTime: number | null;
	text: string;
	translations?: string[];
	karaokeWords?: KaraokeWord[];
}

/**
 * Sentinel end time used for the last line of a line-timed lyric when no
 * following line exists to derive the end time from (mirrors upstream
 * `MAX_LRC_TIMESTAMP`). 999:59.999 in milliseconds.
 */
export const MAX_LRC_TIMESTAMP = 60_039_999;

/**
 * Flat projection: the joined main lyric text of a line.
 */
export function lineText(line: LyricLine): string {
	return line.words.map((w) => w.word).join('');
}

/**
 * Flat projection: translations of a line as a list.
 */
export function lineTranslations(line: LyricLine): string[] {
	const out: string[] = [];
	if (line.translatedLyric) out.push(line.translatedLyric);
	if (line.romanLyric) out.push(line.romanLyric);
	return out;
}

/**
 * Flat projection: per-word karaoke timing of a line.
 */
export function lineKaraokeWords(line: LyricLine): KaraokeWord[] {
	return line.words.map((w) => ({
		startTime: w.startTime,
		endTime: w.endTime,
		text: w.word,
	}));
}

/**
 * Convert a {@link SimpleLyricLine} list into the canonical {@link LyricLine}
 * representation by projecting `text` / `karaokeWords` back into `words`.
 */
export function fromSimpleLyricLines(simple: SimpleLyricLine[]): LyricLine[] {
	return simple.map((s) => {
		const words: LyricWord[] = s.karaokeWords?.length
			? s.karaokeWords.map((k) => ({
					startTime: k.startTime,
					endTime: k.endTime,
					word: k.text,
				}))
			: [
					{
						startTime: s.startTime,
						endTime: s.endTime ?? MAX_LRC_TIMESTAMP,
						word: s.text,
					},
				];
		return {
			words,
			translatedLyric: s.translations?.[0] ?? '',
			romanLyric: s.translations?.[1] ?? '',
			startTime: s.startTime,
			endTime: s.endTime ?? words[words.length - 1].endTime,
			isBG: false,
			isDuet: false,
		};
	});
}
