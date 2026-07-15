// This file is part of utoaudio, licensed under AGPL-3.0.
// Derivative work based on AMLL (https://github.com/amll-dev/applemusic-like-lyrics),
// which is also licensed under AGPL-3.0. See LICENSE for full license text.

/**
 * Lyric player layout helpers — pure functions mirror of AMLL's
 * `LyricPlayerBase` layout / timeline computation, decoupled from Svelte so
 * they are trivially testable.
 *
 * The components pair these pure helpers with one imperative rAF loop (for the
 * single scroll spring + the active line's karaoke mask sweep + interlude
 * dots). Per-line *discrete* style (scale / opacity / blur transitions between
 * "active / passed / upcoming" states) is handled declaratively by Svelte via
 * CSS transitions — this corresponds to AMLL's documented non-spring
 * (`enableSpring=false`) per-line path, which is one of the modes AMLL ships.
 */
import { clamp01 } from "./anim";
import type { LyricLine } from "../../lib/types/lyrics";

/** A group = one main line plus an optional background-vocal line. */
export interface LyricGroup {
  /** Main (non-background) line. */
  main: LyricLine;
  /** Optional background vocal line attached to the previous main line. */
  bg: LyricLine | null;
  /** Window start (ms), `min(main, bg)`. */
  startTime: number;
  /** Window end (ms), `max(main, bg)`. */
  endTime: number;
  /** Whether either line is a duet line. */
  isDuet: boolean;
}

/**
 * Group the lyric lines: an `isBG` line attaches as the background vocal of the
 * previous main line. Mirrors AMLL's `setLyricLines` grouping logic (only one
 * bg per main line — extra consecutive bg lines just follow as their own main
 * lines, matching `convertExcessiveBackgroundLines`).
 */
export function buildGroups(lines: LyricLine[]): LyricGroup[] {
  const groups: LyricGroup[] = [];
  for (const original of lines) {
    const line: LyricLine = {
      words: original.words.map((w) => ({
        ...w,
        ruby: w.ruby ? [...w.ruby] : undefined,
      })),
      translatedLyric: original.translatedLyric,
      romanLyric: original.romanLyric,
      startTime: original.startTime,
      endTime: original.endTime,
      isBG: original.isBG,
      isDuet: original.isDuet,
    };
    if (line.isBG && groups.length > 0) {
      const last = groups[groups.length - 1];
      last.bg = line;
      last.startTime = Math.min(last.startTime, line.startTime);
      last.endTime = Math.max(last.endTime, line.endTime);
      last.isDuet = last.isDuet || line.isDuet;
    } else {
      groups.push({
        main: line,
        bg: null,
        startTime: line.startTime,
        endTime: line.endTime,
        isDuet: line.isDuet,
      });
    }
  }
  return groups;
}

/** Whether the line carries per-word (syllable) timing. */
export function isLineDynamic(line: LyricLine): boolean {
  return line.words.length > 1;
}

/** Whether the whole lyric set is non-dynamic (line-timed only, e.g. LRC). */
export function isNonDynamicSet(groups: LyricGroup[]): boolean {
  return groups.every((g) => g.main.words.length <= 1);
}

/**
 * The active line index for `time`. A group is "hot" when `time` is within its
 * `[startTime, endTime)` window. When no window contains `time`, the next
 * upcoming group is chosen (or the last one if `time` is past the end).
 */
export function findScrollTarget(groups: LyricGroup[], time: number): number {
  if (groups.length === 0) return 0;
  let hot = -1;
  for (let i = 0; i < groups.length; i++) {
    if (groups[i].startTime <= time && groups[i].endTime > time) hot = i;
  }
  if (hot === -1) {
    for (let i = 0; i < groups.length; i++) {
      if (groups[i].startTime >= time) {
        hot = i;
        break;
      }
    }
    if (hot === -1) hot = groups.length - 1;
  }
  return hot;
}

/** Cumulative measured height of groups `[0, index)`. */
export function cumulativeHeight(heights: number[], index: number): number {
  let sum = 0;
  for (let i = 0; i < index && i < heights.length; i++) sum += heights[i];
  return sum;
}

/**
 * The container scroll offset (px, positive) so that the active line's anchor
 * sits at `alignPosition` of the player height. Mirrors AMLL `calcLayout`.
 */
export function computeScrollOffset(
  heights: number[],
  scrollToIndex: number,
  alignPosition: number,
  alignAnchor: "top" | "bottom" | "center",
  playerHeight: number,
): number {
  const before = cumulativeHeight(heights, scrollToIndex);
  const targetHeight = heights[scrollToIndex] ?? playerHeight / 5;
  let offset = before - playerHeight * alignPosition;
  switch (alignAnchor) {
    case "bottom":
      offset -= targetHeight;
      break;
    case "top":
      break;
    case "center":
    default:
      offset -= targetHeight / 2;
      break;
  }
  return offset;
}

/** Discrete per-line presentation derived from the line's position vs the active line. */
export interface LinePresentation {
  /** `true` when this is the active (hot) line. */
  active: boolean;
  /** `true` when this line has already been sung. */
  passed: boolean;
  /** `true` when this line has not started yet. */
  upcoming: boolean;
  /** Scale factor — inactive lines shrink to `0.97` (when scale enabled). */
  scale: number;
  /** Target opacity (active `0.85`, inactive dynamic `0.32`, inactive non-dynamic `0.18`). */
  opacity: number;
  /** Blur radius in px, growing with distance from the active line (clamped to 5). */
  blur: number;
}

/**
 * Per-line visual presentation. Mirrors AMLL's `computeGroupPresentation` plus
 * `computeLineBlur`. The actual smooth motion between states is delegated to
 * CSS `transition` (the non-spring path) by the component.
 */
export function computePresentation(
  index: number,
  scrollToIndex: number,
  opts: {
    playing: boolean;
    enableBlur: boolean;
    enableScale: boolean;
    hidePassedLines: boolean;
    nonDynamic: boolean;
  },
): LinePresentation {
  const active = index === scrollToIndex;
  const passed = index < scrollToIndex;
  const upcoming = index > scrollToIndex;

  let opacity: number;
  if (opts.hidePassedLines) {
    if (passed && opts.playing) opacity = 1e-4;
    else if (active) opacity = 0.85;
    else opacity = opts.nonDynamic ? 0.18 : 0.32;
  } else if (active) {
    opacity = 0.85;
  } else {
    opacity = opts.nonDynamic ? 0.18 : 0.32;
  }

  let blur = 0;
  if (opts.enableBlur && opts.playing && !active) {
    let level = 1;
    if (passed) level += Math.abs(scrollToIndex - index) + 1;
    else if (upcoming) level += Math.abs(index - scrollToIndex);
    const compact = typeof window !== "undefined" && window.innerWidth <= 1024;
    blur = Math.min(5, compact ? level * 0.8 : level);
  }

  const scale = active || !opts.playing ? 1 : opts.enableScale ? 0.97 : 1;
  return { active, passed, upcoming, scale, opacity, blur };
}

/**
 * Karaoke mask-position string for a single word given the active line's left
 * edge and the rendered word spans' offsets/widths.
 *
 * Mirrors AMLL's mask-sweep: a bright window slides left-`fade` → right across
 * the word's `[startTime, endTime]`. Returns `null` when the word is fully
 * outside relevance (before its start → fully hidden, after its end → visible,
 * rendered bright by the caller). The caller clamps to the word box.
 *
 * The returned string is the CSS `mask-position` value (px, 0).
 */
export function wordMaskPosition(
  wordLeft: number,
  wordWidth: number,
  fadePx: number,
  word: { startTime: number; endTime: number },
  time: number,
): string {
  const dur = Math.max(1, Math.abs(word.endTime - word.startTime));
  const t = clamp01((time - word.startTime) / dur);
  const span = wordWidth + fadePx * 2;
  const pos = wordLeft - fadePx + t * span;
  return `${pos.toFixed(2)}px 0px`;
}

/** Interlude description for the in-progress long gap, if any. */
export interface Interlude {
  startTime: number;
  endTime: number;
  anchor: number;
}

/**
 * Detect an in-progress interlude (a >4s gap between lines) at `time`. Mirrors
 * AMLL's `computeCurrentInterlude`.
 */
export function findInterlude(
  groups: LyricGroup[],
  time: number,
): Interlude | null {
  const idx = findScrollTarget(groups, time);
  for (const k of [idx - 1, idx, idx + 1]) {
    if (k < -1 || k + 1 >= groups.length) continue;
    const gapStart = k === -1 ? 0 : groups[k].endTime;
    const gapEnd = Math.max(gapStart, groups[k + 1].startTime - 250);
    if (gapEnd - gapStart < 4000) continue;
    if (time + 20 > gapStart && time + 20 < gapEnd) {
      return {
        startTime: Math.max(gapStart, time),
        endTime: gapEnd,
        anchor: k,
      };
    }
  }
  return null;
}
