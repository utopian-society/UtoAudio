// This file is part of utoaudio, licensed under AGPL-3.0.
// Derivative work based on AMLL (https://github.com/amll-dev/applemusic-like-lyrics),
// which is also licensed under AGPL-3.0. See LICENSE for full license text.

<script module lang="ts">
	/** Spring parameters reused for the single scroll spring. */
	export { defaultPosYSpringParams as scrollSpringParams } from './spring';
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import LyricLine from './LyricLine.svelte';
	import FluidBackground from './FluidBackground.svelte';
	import {
		buildGroups,
		computePresentation,
		computeScrollOffset,
		findInterlude,
		findScrollTarget,
		isNonDynamicSet,
		type LyricGroup,
		type LinePresentation,
	} from './controller';
	import { clamp, clamp01, clampPositive, easeInOutBack, easeOutExpo } from './anim';
	import { Spring, defaultPosYSpringParams } from './spring';
	import type { LyricPlayerProps, LyricTheme } from '../../lib/types/lyrics';

	// --- Props ---------------------------------------------------------------
	interface Props extends LyricPlayerProps {}

	let {
		lyrics,
		currentTime = 0,
		playing = true,
		onLineChange,
		animationMode = 'default',
		theme,
		height,
		width,
		alignPosition = 0.35,
		alignAnchor = 'center',
		enableSpring = true,
		enableBlur = true,
		enableScale = true,
		hidePassedLines = false,
		wordFadeWidth = 0.5,
		isSeeking = false,
		enableFluidBackground = false,
	}: Props = $props();

	// --- Refs ----------------------------------------------------------------
	let playerEl = $state<HTMLDivElement | null>(null);
	let scrollEl = $state<HTMLDivElement | null>(null);
	let interludeEl = $state<HTMLDivElement | null>(null);
	let dot0 = $state<HTMLSpanElement | null>(null);
	let dot1 = $state<HTMLSpanElement | null>(null);
	let dot2 = $state<HTMLSpanElement | null>(null);
	const dotsEls = $derived<[HTMLSpanElement | null, HTMLSpanElement | null, HTMLSpanElement | null]>([dot0, dot1, dot2]);
	const lineEls: (HTMLElement | null)[] = [];

	// --- Derived model -------------------------------------------------------
	const groups = $derived<LyricGroup[]>(buildGroups(lyrics));
	const nonDynamic = $derived(isNonDynamicSet(groups));
	const scrollToIndex = $derived(findScrollTarget(groups, currentTime));
	const fontSize = $derived(
		typeof window !== 'undefined' && window.matchMedia('(max-width: 768px)').matches
			? 'max(8vw, 12px)'
			: 'max(max(5vh, 2.5vw), 12px)',
	);

	// Per-line discrete presentations (driven by scroll target → CSS transitions).
	const presentations = $derived<LinePresentation[]>(
		groups.map((_, i) =>
			computePresentation(i, scrollToIndex, {
				playing,
				enableBlur,
				enableScale,
				hidePassedLines,
				nonDynamic,
			}),
		),
	);
	  const enableKaraoke = $derived(
	    animationMode === 'karaoke' ||
	    (animationMode === 'default' && !nonDynamic),
	  );
	  /** Morph/emphasize glow on long words — on in `default` / `karaoke` modes. */
	  const enableEmphasize = $derived(
	    animationMode === 'default' || animationMode === 'karaoke',
	  );

	// Player size (CSS px). `height`/`width` props win, else measure the element.
	let measuredH = $state(0);
	let measuredW = $state(0);
	const playerH = $derived(height ?? measuredH);
	const playerW = $derived(width ?? measuredW);

	// --- Line height measurement --------------------------------------------
	// Each line's natural height (block-flow); measured after render and on resize.
	const lineHeights = $state<number[]>([]);
	function syncHeights(): void {
		if (!scrollEl) return;
		const children = scrollEl.querySelectorAll<HTMLElement>(':scope > .amll-lyric-line-wrapper');
		const heights: number[] = [];
		children.forEach((c) => heights.push(c.offsetHeight || 0));
		// In-place update so the spring target recomputes.
		lineHeights.splice(0, lineHeights.length, ...heights);
	}

	// --- Scroll spring + rAF loop -------------------------------------------
	const targetScroll = $derived(
		groups.length === 0 ? 0 : computeScrollOffset(lineHeights, scrollToIndex, alignPosition, alignAnchor, playerH),
	);
	let scrollSpring = new Spring(0, defaultPosYSpringParams);
	// Re-create the spring on enableSpring toggle so it always reflects the mode.
	$effect(() => {
		// Read enableSpring to track changes.
		const enabled = enableSpring;
		if (!enabled) scrollSpring.setPositionInstant(scrollSpring.getCurrentPosition());
	});

	// Active-line word-mask state for the imperative per-frame writes.
	const fadePx = $derived(wordFadeWidth * 32); // ~ font px; tuned to the em-based font size
	const interlude = $derived(findInterlude(groups, currentTime));

	let lastLine = -1;
	$effect(() => {
		if (scrollToIndex !== lastLine && scrollToIndex >= 0) {
			lastLine = scrollToIndex;
			onLineChange?.(scrollToIndex);
		}
	});

	// The one rAF loop: scroll spring, active-line karaoke masks, interlude dots.
	$effect(() => {
		if (!scrollEl || groups.length === 0) return;
		const scroller = scrollEl; // non-null alias for the closure
		let raf = 0;
		let prev = performance.now();
		const loop = (now: number) => {
			const dtMs = Math.min(64, now - prev);
			prev = now;
			// 1) Scroll spring toward target.
			if (enableSpring) {
				scrollSpring.setTargetPosition(targetScroll, 0);
				scrollSpring.update(dtMs / 1000);
			} else {
				scrollSpring.setPositionInstant(targetScroll);
			}
			scroller.style.transform = `translateY(${(-scrollSpring.getCurrentPosition()).toFixed(2)}px)`;

			// 2) Karaoke mask sweep on the active line's words (imperative per-frame).
			const activeEl = lineEls[scrollToIndex];
			if (activeEl && (enableKaraoke || !nonDynamic)) {
				const wordSpans = activeEl.querySelectorAll<HTMLElement>('[data-word]');
				let cursor = 0;
				wordSpans.forEach((wordEl, wi) => {
					const startAttr = wordEl.dataset.start;
					const endAttr = wordEl.dataset.end;
					if (startAttr == null || endAttr == null) return;
					const wordStart = Number(startAttr);
					const wordEnd = Number(endAttr);
					const w = wordEl.offsetWidth;
					const pos = maskPos(cursor, w, fadePx, wordStart, wordEnd, currentTime);
					wordEl.style.maskPosition = pos;
					wordEl.style.webkitMaskPosition = pos;
					// Each word also bumps the active morph duration var.
					wordEl.style.setProperty('--word-duration', `${Math.max(1000, wordEnd - wordStart)}ms`);
					cursor += w;
				});
			}

			// 3) Interlude dots animation.
			styleInterludeDots(currentTime);

			raf = requestAnimationFrame(loop);
		};
		raf = requestAnimationFrame(loop);
		return () => cancelAnimationFrame(raf);
	});

	// Re-measure heights on lyric / size / font changes, scheduled post-render.
	$effect(() => {
		// touch deps
		lyrics;
		playerH;
		playerW;
		groups.length;
		// defer to next frame so DOM reflects the latest structure
		const id = requestAnimationFrame(syncHeights);
		return () => cancelAnimationFrame(id);
	});

	// Recompute interlude dot transforms each frame is too costly through
	// Svelte reactivity; the rAF loop calls this imperative helper.
	function styleInterludeDots(time: number): void {
		if (!interludeEl) return;
		const cur = interlude;
		if (!cur || !interludeEl) {
			return;
		}
		const dur = cur.endTime - cur.startTime;
		const elapsed = time - cur.startTime;
		const breatheDur = dur / Math.ceil(dur / 1500);
		let scale = 1;
		scale *= Math.sin(1.5 * Math.PI - (elapsed / breatheDur) * 2) / 20 + 1;
		if (elapsed < 2000) scale *= easeOutExpo(clamp01(elapsed / 2000));
		let globalOpacity = 1;
		if (elapsed < 500) globalOpacity = 0;
		else if (elapsed < 1000) globalOpacity = (elapsed - 500) / 500;
		const remaining = dur - elapsed;
		if (remaining < 750) scale *= 1 - easeInOutBack((750 - remaining) / 750 / 2);
		if (remaining < 375) globalOpacity *= clamp01(remaining / 375);
		scale = clampPositive(scale) * 0.7;
		interludeEl.style.opacity = globalOpacity.toFixed(3);
		interludeEl.style.transform = `scale(${scale.toFixed(3)})`;
		const dotsDur = clampPositive(dur - 750);
		dotsEls.forEach((dot, di) => {
			if (!dot) return;
			const base = (elapsed - (dotsDur / 3) * di) * 3;
			const alpha = clamp(0.25, (base / Math.max(1, dotsDur)) * 0.75, 1);
			dot.style.opacity = alpha.toFixed(3);
		});
	}

	/** Compute one word's mask-position string. Mirrors AMLL's mask sweep. */
	function maskPos(
		left: number,
		width: number,
		fade: number,
		start: number,
		end: number,
		time: number,
	): string {
		const dur = Math.max(1, Math.abs(end - start));
		const t = clamp01((time - start) / dur);
		const span = width + fade * 2;
		const pos = left - fade + t * span;
		return `${pos.toFixed(2)}px 0px`;
	}

	// --- ResizeObserver for the player element -------------------------------
	onMount(() => {
		if (!playerEl) return;
		const ro = new ResizeObserver(() => {
			const r = playerEl!.getBoundingClientRect();
			measuredH = r.height;
			measuredW = r.width;
			syncHeights();
		});
		ro.observe(playerEl);
		const r = playerEl.getBoundingClientRect();
		measuredH = r.height;
		measuredW = r.width;
		return () => ro.disconnect();
	});

	// --- Wire child line refs ------------------------------------------------
	function handleLineMount(el: HTMLElement, index: number): void {
		lineEls[index] = el;
	}
	function handleLineDestroy(index: number): void {
		if (lineEls[index]) lineEls[index] = null;
	}

	// --- Gestures: swipe-to-pause + tap-to-toggle-fullscreen -----------------
	let isFullscreen = $state(false);
	function onPointerDown(e: PointerEvent): void {
		const startX = e.clientX;
		const startY = e.clientY;
		const startT = performance.now();
		let moved = false;
		const onMove = (ev: Event) => {
			const pe = ev as PointerEvent;
			if (Math.abs(pe.clientX - startX) + Math.abs(pe.clientY - startY) > 12) moved = true;
		};
		const onUp = (ev: Event) => {
			const pe = ev as PointerEvent;
			window.removeEventListener('pointermove', onMove);
			window.removeEventListener('pointerup', onUp);
			const dt = performance.now() - startT;
			if (!moved && dt < 250) {
				// Tap → toggle full-screen.
				isFullscreen = !isFullscreen;
			} else if (moved && Math.abs(pe.clientY - startY) > 60) {
				// Vertical swipe → toggle the play gesture (host decides on pause).
				playing = !playing;
			}
		};
		window.addEventListener('pointermove', onMove);
		window.addEventListener('pointerup', onUp);
	}

	// --- Theme application ---------------------------------------------------
	$effect(() => {
		if (!playerEl) return;
		const t = theme as LyricTheme | undefined;
		playerEl.style.setProperty('--amll-lp-color', t?.color ?? '#ffffff');
	});

</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	bind:this={playerEl}
	class="amll-lyric-player"
	class:fullscreen={isFullscreen}
	class:light={theme?.light ?? false}
	class:seeking={isSeeking}
	onpointerdown={onPointerDown}
	style:--amll-lp-font-size={fontSize}
	style:width={width != null ? `${width}px` : null}
	style:height={height != null ? `${height}px` : null}
>
	{#if enableFluidBackground}
		<FluidBackground theme={theme} playing={playing} flowSpeed={2} renderScale={0.5} staticMode={!playing} />
	{/if}

	<div class="amll-lyric-scroll" bind:this={scrollEl}>
		{#each groups as group, i (i)}
			<LyricLine
				{group}
				presentation={presentations[i]}
				{enableKaraoke}
				{enableEmphasize}
				index={i}
				onmount={handleLineMount}
				ondestroy={handleLineDestroy}
			/>
		{/each}

		<div
			class="amll-interlude-dots"
			class:enabled={interlude !== null}
			class:playing={playing}
			bind:this={interludeEl}
		>
			<span bind:this={dot0}></span>
			<span bind:this={dot1}></span>
			<span bind:this={dot2}></span>
		</div>
	</div>
</div>

<style>
	.amll-lyric-player {
		position: relative;
		width: 100%;
		height: 100%;
		overflow: hidden;
		color: var(--amll-lp-color, #ffffff);
		font-size: var(--amll-lp-font-size, max(max(5vh, 2.5vw), 12px));
		line-height: 1.2;
		user-select: none;
		contain: strict;
		mix-blend-mode: plus-lighter;
		--amll-lp-width: 100%;
		--lyric-line-padding-x: 1em;
	}
	.amll-lyric-player.light {
		--amll-lp-color: #111111;
	}
	.amll-lyric-player.fullscreen {
		position: fixed;
		inset: 0;
		z-index: 50;
		width: 100vw;
		height: 100vh;
	}
	.amll-lyric-scroll {
		position: absolute;
		left: 0;
		right: 0;
		top: 0;
		display: flex;
		flex-direction: column;
		gap: 0.3em;
		padding: 0 0.4em var(--lyric-line-padding-x);
		will-change: transform;
	}
	.amll-interlude-dots {
		position: absolute;
		left: 0;
		bottom: 0;
		display: none;
		gap: 0.25em;
		width: fit-content;
		padding: 2.5% 0.75em;
		opacity: 0;
		transform-origin: center;
		transition: opacity 0.25s ease;
	}
	.amll-interlude-dots.enabled {
		display: flex;
		opacity: 1;
	}
	.amll-interlude-dots span {
		width: clamp(0.5em, 1vh, 3em);
		aspect-ratio: 1 / 1;
		border-radius: 50%;
		background-color: var(--amll-lp-color, #ffffff);
		opacity: 0.25;
	}
</style>
