<script lang="ts">
	/** Names of the icons this component knows how to render. */
	export type IconName =
		| 'speaker'
		| 'play'
		| 'pause'
		| 'skip-next'
		| 'skip-prev'
		| 'playlist'
		| 'library'
		| 'folder'
		| 'gear'
		| 'music'
		| 'plus'
		| 'chevron-down'
		| 'chevron-right'
		| 'close'
		| 'minimize'
		| 'search'
		| 'rescan'
		| 'queue-add'
		| 'arrow-up'
		| 'arrow-down'
		| 'info'
		| 'appearance'
		| 'eq'
		| 'check'
		| 'volume-low'
		| 'repeat'
		| 'repeat-one'
		| 'shuffle'
		| 'queue-list';

	interface Props {
		name: IconName;
		/** Square size in CSS pixels (both width and height). */
		size?: number;
		/** Optional extra class names appended to the rendered `<svg>`. */
		class?: string;
		/** Stroke width — defaults to the feather/lucide 1.75 value. */
		strokeWidth?: number;
		/** Optional title for accessibility. When set, `aria-hidden` is
		 * removed from the svg and a `<title>` element is rendered so screen
		 * readers announce the icon. */
		title?: string;
	}

	let {
		name,
		size = 18,
		class: klass = '',
		strokeWidth = 1.75,
		title,
	}: Props = $props();

	/**
	 * `name → inner SVG markup`. Each entry is the body of a 24x24 viewBox
	 * svg that follows the feather/lucide stroke convention. The outer
	 * `<svg>` element is added by the component below so the
	 * `fill`/`stroke` props and `class` can be merged consistently.
	 */
	const PATHS: Record<IconName, string> = {
		speaker:
			'<polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>',
		play: '<polygon points="6 4 20 12 6 20 6 4"/>',
		pause:
			'<rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/>',
		'skip-next':
			'<polygon points="5 4 15 12 5 20 5 4"/><line x1="19" y1="5" x2="19" y2="19"/>',
		'skip-prev':
			'<polygon points="19 20 9 12 19 4 19 20"/><line x1="5" y1="19" x2="5" y2="5"/>',
		playlist:
			'<path d="M3 6h13"/><path d="M3 12h13"/><path d="M3 18h9"/><path d="M16 14V9l6 -2v5"/><circle cx="16" cy="18" r="2"/><circle cx="22" cy="16" r="2"/>',
		library:
			'<line x1="3" y1="20" x2="3" y2="4"/><line x1="7.5" y1="20" x2="7.5" y2="4"/><rect x="11" y="4" width="10" height="16" rx="1"/><line x1="14" y1="9" x2="18" y2="9"/>',
		folder:
			'<path d="M2 6a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6z"/>',
		gear: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>',
		music:
			'<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>',
		plus:
			'<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>',
		'chevron-down': '<polyline points="6 9 12 15 18 9"/>',
		'chevron-right': '<polyline points="9 18 15 12 9 6"/>',
		close:
			'<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>',
		minimize: '<line x1="5" y1="12" x2="19" y2="12"/>',
		search:
			'<circle cx="11" cy="11" r="7"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>',
		rescan:
			'<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/>',
		'queue-add':
			'<path d="M3 6h10"/><path d="M3 12h7"/><path d="M3 18h7"/><line x1="15" y1="13" x2="21" y2="13"/><line x1="18" y1="10" x2="18" y2="16"/>',
		'arrow-up':
			'<line x1="12" y1="19" x2="12" y2="5"/><polyline points="5 12 12 5 19 12"/>',
		'arrow-down':
			'<line x1="12" y1="5" x2="12" y2="19"/><polyline points="19 12 12 19 5 12"/>',
		info:
			'<circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/>',
		appearance:
			'<circle cx="12" cy="12" r="4"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="m4.93 4.93 1.41 1.41"/><path d="m17.66 17.66 1.41 1.41"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="m6.34 17.66-1.41 1.41"/><path d="m19.07 4.93-1.41 1.41"/>',
		eq: '<line x1="4" y1="21" x2="4" y2="14"/><line x1="4" y1="10" x2="4" y2="3"/><line x1="12" y1="21" x2="12" y2="12"/><line x1="12" y1="8" x2="12" y2="3"/><line x1="20" y1="21" x2="20" y2="16"/><line x1="20" y1="12" x2="20" y2="3"/><line x1="1" y1="14" x2="7" y2="14"/><line x1="9" y1="8" x2="15" y2="8"/><line x1="17" y1="16" x2="23" y2="16"/>',
		check: '<polyline points="20 6 9 17 4 12"/>',
		'volume-low':
			'<polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/>',
		repeat:
			'<polyline points="17 1 21 5 17 9"/><path d="M3 11V9a4 4 0 0 1 4-4h14"/><polyline points="7 23 3 19 7 15"/><path d="M21 13v2a4 4 0 0 1-4 4H3"/>',
		'repeat-one':
			'<polyline points="17 1 21 5 17 9"/><path d="M3 11V9a4 4 0 0 1 4-4h14"/><polyline points="7 23 3 19 7 15"/><path d="M21 13v2a4 4 0 0 1-4 4H3"/><path d="M11 10h1v4"/>',
		shuffle:
			'<polyline points="16 3 21 3 21 8"/><line x1="4" y1="20" x2="21" y2="3"/><polyline points="21 16 21 21 16 21"/><line x1="15" y1="15" x2="21" y2="21"/><line x1="4" y1="4" x2="9" y2="9"/>',
		'queue-list':
			'<line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/>',
	};

	// Resolve once when props change; avoids re-evaluating the shape every
	// render tick.
	const inner = $derived(PATHS[name] ?? '');
</script>

<svg
	xmlns="http://www.w3.org/2000/svg"
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width={strokeWidth}
	stroke-linecap="round"
	stroke-linejoin="round"
	width={size}
	height={size}
	class={klass}
	aria-hidden={title ? undefined : 'true'}
	role={title ? 'img' : undefined}
>
	{#if title}<title>{title}</title>{/if}
	<!-- Inline-render the per-name path markup. Svelte preserves raw nested
	     elements when @html is used; sanitised-by-construction since the
	     only strings pushed through it come from the static `PATHS` map. -->
	{@html inner}
</svg>
