<script lang="ts">
	import { listFiles, type FileMeta } from './api';
	import { formatSize } from './format';

	let { folder, onback }: { folder: string; onback: () => void } = $props();

	let recursive = $state(false);
	let files = $state<FileMeta[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let sortMode = $state<'filename' | 'path'>('filename');
	let collapsed = $state<Record<string, boolean>>({});

	async function load() {
		loading = true;
		try {
			const r = await listFiles(folder);
			files = r.files;
			recursive = r.recursive;
			error = null;
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}
	load();

	function basename(p: string): string {
		const parts = p.split('/').filter(Boolean);
		return parts[parts.length - 1] ?? p;
	}

	function parentDir(rel: string): string {
		const i = rel.lastIndexOf('/');
		return i < 0 ? '' : rel.slice(0, i + 1);
	}

	const sortedByFilename = $derived(
		[...files].sort((a, b) => basename(a.rel).localeCompare(basename(b.rel)))
	);

	type Group = { dir: string; files: FileMeta[] };
	const groupedByPath = $derived.by<Group[]>(() => {
		const map = new Map<string, FileMeta[]>();
		for (const f of [...files].sort((a, b) => a.rel.localeCompare(b.rel))) {
			const dir = parentDir(f.rel);
			if (!map.has(dir)) map.set(dir, []);
			map.get(dir)!.push(f);
		}
		const dirs = [...map.keys()].sort();
		return dirs.map(dir => ({ dir, files: map.get(dir)! }));
	});

	function toggle(dir: string) {
		collapsed = { ...collapsed, [dir]: !collapsed[dir] };
	}
</script>

<header class="page">
	<button class="back" onclick={onback}>← dlna-rs</button>
</header>

<section class="hero">
	<div class="row">
		<h2>{basename(folder)}</h2>
		{#if recursive}<span class="tag">recursive</span>{/if}
		<span class="count">{files.length} files</span>
	</div>
	<div class="path">{folder}</div>
</section>

<nav class="sort">
	<span class="label">sort:</span>
	<label><input type="radio" name="sort" value="filename" bind:group={sortMode} /> filename</label>
	<label><input type="radio" name="sort" value="path" bind:group={sortMode} /> path</label>
</nav>

{#if loading}
	<p>Loading…</p>
{:else if error}
	<p class="err">{error}</p>
{:else if sortMode === 'filename'}
	<ul class="files">
		{#each sortedByFilename as f (f.rel)}
			<li>
				<span class="name">{basename(f.rel)}</span>
				<span class="size">{formatSize(f.size)}</span>
			</li>
		{/each}
	</ul>
{:else}
	{#each groupedByPath as g (g.dir)}
		{#if g.dir === ''}
			<ul class="files">
				{#each g.files as f (f.rel)}
					<li>
						<span class="name">{f.rel}</span>
						<span class="size">{formatSize(f.size)}</span>
					</li>
				{/each}
			</ul>
		{:else}
			<button class="group-head" onclick={() => toggle(g.dir)}>
				<span class="chev">{collapsed[g.dir] ? '▸' : '▾'}</span>
				<code>{g.dir}</code>
				<span class="count">{g.files.length} files</span>
			</button>
			{#if !collapsed[g.dir]}
				<ul class="files indent">
					{#each g.files as f (f.rel)}
						<li>
							<span class="name">{f.rel.slice(g.dir.length)}</span>
							<span class="size">{formatSize(f.size)}</span>
						</li>
					{/each}
				</ul>
			{/if}
		{/if}
	{/each}
{/if}

<style>
	.page { margin-bottom: 1rem; }
	.back { background: none; border: none; padding: 0; font: inherit; color: #555; cursor: pointer; }
	.back:hover { color: #000; }

	.hero { padding: 0.5rem 0 1rem; border-bottom: 1px solid #eee; }
	.hero .row { display: flex; align-items: baseline; gap: 0.75rem; }
	.hero h2 { font-size: 1.05rem; margin: 0; }
	.tag { font-size: 0.7rem; color: #888; background: #f3f3f3; padding: 0.05rem 0.4rem; border-radius: 3px; }
	.count { font-variant-numeric: tabular-nums; color: #666; font-size: 0.85rem; margin-left: auto; }
	.path { color: #999; font-family: ui-monospace, monospace; font-size: 0.8rem; margin-top: 0.2rem; }

	.sort { display: flex; gap: 1rem; align-items: center; padding: 0.6rem 0; font-size: 0.9rem; }
	.sort .label { color: #888; }

	.files { list-style: none; padding: 0; margin: 0; }
	.files li {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: 1rem;
		padding: 0.2rem 0;
		font-size: 0.9rem;
	}
	.files .name { font-family: ui-monospace, monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.files .size { color: #777; font-variant-numeric: tabular-nums; }
	.indent { padding-left: 1rem; }

	.group-head {
		display: flex; align-items: baseline; gap: 0.5rem;
		background: none; border: none; padding: 0.5rem 0 0.2rem;
		font: inherit; cursor: pointer; width: 100%; text-align: left;
		color: #444;
	}
	.group-head:hover { color: #000; }
	.group-head .chev { width: 1rem; color: #999; }
	.group-head code { font-size: 0.85rem; }
	.group-head .count { margin-left: 0; color: #999; font-size: 0.8rem; }

	.err { color: #c33; }
</style>
