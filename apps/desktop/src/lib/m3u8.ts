// This file is part of utoaudio, licensed under AGPL-3.0.
//
// `m3u8` — pure-TypeScript parser / serializer for the m3u8 / M3U playlist
// format used by utoaudio's Playlist page.
//
// Handles the subset of m3u8 we care about:
//   - `#EXTM3U` header (optional but tolerated).
//   - `#PLAYLIST:<name>` header (optional, captured into `playlistName`).
//   - `#EXTINF:<duration>,<title>` per-track metadata (duration in seconds
//     as a float, title as a free-form string that may contain commas).
//   - Blank lines and lines starting with `#` other than the above are
//     ignored.
//   - File paths: absolute (anything containing a path separator or starting
//     with `/`) are preserved verbatim; relative paths are resolved against
//     `baseDir` when provided.
//
// The module is dependency-free and safe to import from any page.

/** A single entry in an m3u8 playlist. */
export interface M3u8Track {
	/** Filesystem path to the audio file (absolute when resolvable). */
	path: string;
	/** Optional display title (taken from `#EXTINF`'s title field). */
	title?: string;
	/** Optional artist (parsed from `Artist - Title` form when present). */
	artist?: string;
	/** Optional track length in seconds (from `#EXTINF`'s duration field). */
	duration?: number;
}

/**
 * Parse an m3u8 document into a list of tracks.
 *
 * @param content  Raw m3u8 file contents (UTF-8).
 * @param baseDir  Optional base directory used to resolve relative paths.
 *                 When omitted, relative paths are preserved as-is.
 * @returns        Parsed tracks in document order.
 */
export function parseM3u8(content: string, baseDir?: string): M3u8Track[] {
	// Normalize line endings — m3u8 files in the wild use either `\n` or `\r\n`.
	const lines = content.replace(/\r\n?/g, '\n').split('\n');

	const tracks: M3u8Track[] = [];
	// The most recently seen `#EXTINF` line, waiting for its path line.
	let pendingDuration: number | undefined;
	let pendingTitle: string | undefined;

	for (const rawLine of lines) {
		const line = rawLine.trim();
		if (line === '') {
			// Blank line — flush any dangling EXTINF.
			pendingDuration = undefined;
			pendingTitle = undefined;
			continue;
		}

		if (line.startsWith('#EXTINF')) {
			// `#EXTINF:<duration>,<title>` — duration is up to the first comma.
			const rest = line.slice('#EXTINF'.length).replace(/^:/, '');
			const commaIdx = rest.indexOf(',');
			if (commaIdx >= 0) {
				const durationStr = rest.slice(0, commaIdx).trim();
				const titleStr = rest.slice(commaIdx + 1).trim();
				const parsed = Number.parseFloat(durationStr);
				pendingDuration = Number.isFinite(parsed) ? parsed : undefined;
				pendingTitle = titleStr.length > 0 ? titleStr : undefined;
			} else {
				// No comma — treat the whole thing as a (possibly numeric) title.
				const parsed = Number.parseFloat(rest.trim());
				pendingDuration = Number.isFinite(parsed) ? parsed : undefined;
				pendingTitle = rest.trim().length > 0 ? rest.trim() : undefined;
			}
			continue;
		}

		if (line.startsWith('#')) {
			// Other directives (`#EXTM3U`, `#PLAYLIST:…`, unknown `#…`) are
			// intentionally ignored here — the caller may pass `playlistName`
			// separately if it needs to capture the `#PLAYLIST` header.
			continue;
		}

		// Path line — pair with the most recent `#EXTINF` (if any) and emit.
		const resolved = resolvePath(line, baseDir);
		const { artist, title } = splitArtistTitle(pendingTitle);
		tracks.push({
			path: resolved,
			title,
			artist,
			duration: pendingDuration,
		});
		pendingDuration = undefined;
		pendingTitle = undefined;
	}

	return tracks;
}

/**
 * Serialize a list of tracks back into a valid m3u8 document.
 *
 * The output uses `#EXTM3U` as the first line (a common convention) and
 * emits one `#EXTINF` block per track, even for tracks that lack duration
 * or title metadata.
 *
 * @param tracks         Tracks to write out.
 * @param playlistName   Optional name; when supplied a `#PLAYLIST:` header
 *                       is emitted after `#EXTM3U`.
 * @returns              The serialized m3u8 document (LF line endings).
 */
export function stringifyM3u8(tracks: M3u8Track[], playlistName?: string): string {
	const lines: string[] = ['#EXTM3U'];
	if (playlistName && playlistName.trim().length > 0) {
		lines.push(`#PLAYLIST:${playlistName.trim()}`);
	}

	for (const track of tracks) {
		const duration = typeof track.duration === 'number' && Number.isFinite(track.duration)
			? Math.max(0, Math.round(track.duration))
			: -1;
		const title = composeTitle(track.title, track.artist);
		lines.push(`#EXTINF:${duration},${title}`);
		lines.push(track.path);
	}

	// Trailing newline so most parsers (including our own) accept the file.
	return lines.join('\n') + '\n';
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/**
 * Resolve `path` against `baseDir` when it's relative; return absolute paths
 * unchanged. With no baseDir, relative paths are preserved verbatim.
 */
function resolvePath(path: string, baseDir?: string): string {
	if (!baseDir) return path;
	const trimmedBase = baseDir.replace(/\/+$/, '');
	const isAlreadyAbsolute = path.startsWith('/') || /^[a-zA-Z]:[\\/]/.test(path);
	if (isAlreadyAbsolute) return path;
	return `${trimmedBase}/${path}`;
}

/**
 * Split a combined `Artist - Title` string into separate fields when possible.
 * The dash must be surrounded by whitespace so that real hyphens inside
 * titles (e.g. "X-Men Theme") don't get split.
 */
function splitArtistTitle(input: string | undefined): { artist?: string; title?: string } {
	if (!input) return {};
	const match = input.match(/^(.+?)\s+-\s+(.+)$/);
	if (match) {
		return { artist: match[1].trim(), title: match[2].trim() };
	}
	return { title: input };
}

/**
 * Combine optional artist + title into the single comma-bearing string that
 * `#EXTINF`'s title field expects.
 */
function composeTitle(title: string | undefined, artist: string | undefined): string {
	if (title && artist) return `${artist} - ${title}`;
	if (title) return title;
	if (artist) return artist;
	return '';
}
