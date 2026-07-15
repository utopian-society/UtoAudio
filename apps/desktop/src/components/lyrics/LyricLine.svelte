// This file is part of utoaudio, licensed under AGPL-3.0.
// Derivative work based on AMLL (https://github.com/amll-dev/applemusic-like-lyrics),
// which is also licensed under AGPL-3.0. See LICENSE for full license text.

<script module lang="ts">
	import { lineText } from '../../lib/types/lyrics';
	import type { LyricGroup } from './controller';
	import type { LinePresentation } from './controller';

	/** Cast a (maybe undefined) callback arg to the closest localized string. */
	export function fmtText(words: { word: string }[]): string {
		return words.map((w) => w.word).join('');
	}
</script>

<script lang="ts">
	import { isCJK } from './anim';

	interface Props {
		/** This line's group (main + optional bg). */
		group: LyricGroup;
		/** Discrete presentation derived by the parent. */
		presentation: LinePresentation;
		/** Whether the per-word karaoke sweep should be shown for this line. */
		enableKaraoke: boolean;
		/** Register the line's root element with the parent's imperative loop. */
		onmount: (el: HTMLElement, index: number) => void;
		/** Called on destroy to release the element. */
		ondestroy: (index: number) => void;
		/** Group index within the lyric player. */
		index: number;
		/** Whether to render the long-word "emphasize" (morph) glow effect. */
		enableEmphasize: boolean;
	}

	let {
		group,
		presentation,
		enableKaraoke,
		onmount,
		ondestroy,
		index,
		enableEmphasize,
	}: Props = $props();

	/** Duration threshold (ms) / length thresholds for the emphasize morph. */
	const EMPHASIZE_MIN_DURATION = 1000;
	const EMPHASIZE_LATIN_MAX_LEN = 7;
	const EMPHASIZE_LATIN_MIN_LEN = 2;

	/** Whether a word qualifies for the morph/emphasize glow (AMLL `shouldEmphasize`). */
	function shouldEmphasize(word: { word: string; startTime: number; endTime: number }): boolean {
		const duration = word.endTime - word.startTime;
		if (duration < EMPHASIZE_MIN_DURATION) return false;
		const text = word.word.trim();
		if (text.length === 0) return false;
		// CJK: only the duration gate matters; non-CJK also requires 2..7 chars.
		if (Array.from(text).some((c) => isCJK(c))) return true;
		return text.length >= EMPHASIZE_LATIN_MIN_LEN && text.length <= EMPHASIZE_LATIN_MAX_LEN;
	}

	const main = $derived(group.main);
	const bg = $derived(group.bg);
	// Whether this line carries per-word syllable timing (drives word spans + mask).
	const dynamic = $derived(main.words.length > 1);
	const isNonDynamicSparse = $derived(!dynamic);
	// When non-dynamic, render a single text node; otherwise render word spans.
	const mainText = $derived(lineText(main));
	// Inline style for the discrete per-line presentation (CSS transitions animate it).
	const lineStyle = $derived({
		transform: `scale(${presentation.scale.toFixed(4)})`,
		opacity: presentation.opacity,
		filter: `blur(${Math.min(5, presentation.blur).toFixed(2)}px)`,
	});

	// Emit ref so the parent can drive per-frame karaoke masks imperatively.
	let rootEl = $state<HTMLElement | null>(null);
	$effect(() => {
		if (rootEl) onmount(rootEl, index);
		return () => ondestroy(index);
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	bind:this={rootEl}
	class="amll-lyric-line-wrapper"
	class:active={presentation.active}
	class:passed={presentation.passed}
	class:is-duet={group.isDuet}
	style:transform={lineStyle.transform}
	style:opacity={lineStyle.opacity}
	style:filter={lineStyle.filter}
>
	<div class="amll-lyric-line" class:duet={group.isDuet} class:bg={bg !== null}>
		<div class="amll-lyric-main-line">
			{#if isNonDynamicSparse}
				<span class="amll-lyric-text">{mainText}</span>
			{:else}
				{#each main.words as word, i (i)}
					<span
						class="amll-lyric-word"
						class:emphasize={enableEmphasize && shouldEmphasize(word)}
						data-word
						data-start={word.startTime}
						data-end={word.endTime}
					>{word.word}{#if word.ruby && word.ruby.length > 0}
						<span class="amll-lyric-ruby">
							{#each word.ruby as r (r.startTime)}<span class="ruby-text">{r.text}</span>{/each}
						</span>
					{/if}{#if word.romanWord}
						<span class="amll-lyric-roman">{word.romanWord}</span>
					{/if}</span>
				{/each}
			{/if}
		</div>
		{#if main.translatedLyric}
			<div class="amll-lyric-sub-line">{main.translatedLyric}</div>
		{/if}
		{#if main.romanLyric}
			<div class="amll-lyric-sub-line roman">{main.romanLyric}</div>
		{/if}
	</div>

	{#if bg}
		<div class="amll-lyric-bg-wrapper" data-bg-wrapper>
			<div class="amll-lyric-main-line">
				{#each bg.words as word, i (i)}
					<span class="amll-lyric-word" data-word data-start={word.startTime} data-end={word.endTime}>{word.word}</span>
				{/each}
			</div>
		</div>
	{/if}
</div>

<style>
	.amll-lyric-line-wrapper {
		position: relative;
		width: 100%;
		will-change: transform, opacity, filter;
		/* smooth discrete state transitions (AMLL non-spring path) */
		transition: transform 0.5s cubic-bezier(0.22, 0.61, 0.36, 1),
			opacity 0.3s ease, filter 0.25s ease;
		transform-origin: left center;
	}
	.amll-lyric-line-wrapper.is-duet,
	.amll-lyric-line-wrapper.is-duet .amll-lyric-line {
		text-align: right;
		transform-origin: right center;
	}
	.amll-lyric-line {
		display: block;
		min-width: 0;
		max-width: var(--amll-lp-width, 100%);
		padding: 0.2em;
		margin: -0.2em;
		contain: layout style paint;
		content-visibility: auto;
		contain-intrinsic-size: auto 2em;
		line-height: 1.2;
	}
	.amll-lyric-main-line {
		display: block;
	}
	.amll-lyric-text,
	.amll-lyric-word {
		display: inline;
		white-space: pre-wrap;
	}
	/* Karaoke mask: bright window slides via mask-position (set imperatively per frame). */
	.amll-lyric-word {
		--bright-mask-alpha: 1;
		--dark-mask-alpha: 0.2;
		-webkit-mask-image: linear-gradient(
			to right,
			rgba(0, 0, 0, var(--bright-mask-alpha, 1)) 40%,
			rgba(0, 0, 0, var(--dark-mask-alpha, 0.2)) 60%
		);
		mask-image: linear-gradient(
			to right,
			rgba(0, 0, 0, var(--bright-mask-alpha, 1)) 40%,
			rgba(0, 0, 0, var(--dark-mask-alpha, 0.2)) 60%
		);
		-webkit-mask-size: 300% 100%;
		mask-size: 300% 100%;
		-webkit-mask-repeat: no-repeat;
		mask-repeat: no-repeat;
		-webkit-mask-origin: left;
		mask-origin: left;
	}
	/* The non-dynamic (whole-line) text is always fully visible; no mask. */
	.amll-lyric-text {
		opacity: 1;
	}
	/* Inactive lines: when not active, drop the mask so the word reads cleanly. */
	.amll-lyric-line-wrapper:not(.active) :global([data-word]) {
		-webkit-mask-image: none;
		mask-image: none;
	}
	/* Emphasize (morph): long words glow + pop. Pulsed via a CSS keyframe driven
	   by the word's duration so it stays dependency-free. */
	.amll-lyric-word.emphasize {
		animation: amll-emphasize-bump var(--word-duration, 2s) ease-in-out 1;
		display: inline-block;
		text-shadow: 0 0 0 rgba(255, 255, 255, 0);
	}
	.amll-lyric-ruby {
		display: inline-flex;
		flex-direction: column-reverse;
		font-size: 0.5em;
		vertical-align: top;
		line-height: 1em;
	}
	.amll-lyric-roman {
		display: inline;
		font-size: 0.5em;
		opacity: 0.7;
		padding-inline-end: 0.3em;
	}
	.amll-lyric-sub-line {
		opacity: 0.3;
		font-size: max(0.5em, 10px);
		line-height: 1.5em;
		transition: opacity 0.2s 0.25s;
	}
	.amll-lyric-sub-line.roman {
		opacity: 0.25;
	}
	/* Background vocal: under (or above) the main line, 0.7em font, slides via the parent's rAF. */
	.amll-lyric-bg-wrapper {
		position: absolute;
		left: 0;
		width: 100%;
		top: calc(100% + 0.3em);
		font-size: 0.7em;
		opacity: 0.4;
		transform-origin: left top;
		will-change: transform, opacity;
		transition: opacity 0.3s ease;
	}
	.amll-lyric-line-wrapper.active .amll-lyric-bg-wrapper {
		opacity: 0.4;
	}

	@keyframes amll-emphasize-bump {
		0% {
			transform: scale(1);
			text-shadow: 0 0 0 rgba(255, 255, 255, 0);
		}
		50% {
			transform: scale(1.06);
			text-shadow: 0 0 0.25em rgba(255, 255, 255, 0.6);
		}
		100% {
			transform: scale(1.06);
			text-shadow: 0 0 0.25em rgba(255, 255, 255, 0.4);
		}
	}
</style>
