<script lang="ts">
	import { browse, addFolder, type BrowseResp } from './api';

	let { onclose, onadded }: { onclose: () => void; onadded: () => void } = $props();

	let current = $state<BrowseResp | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let recursive = $state(true);

	async function go(path?: string) {
		loading = true;
		error = null;
		try { current = await browse(path); }
		catch (e) { error = String(e); }
		finally { loading = false; }
	}
	go();

	async function select() {
		if (!current) return;
		try {
			await addFolder(current.path, recursive);
			onadded();
		} catch (e) { error = String(e); }
	}
</script>

<div class="backdrop" onclick={onclose} role="presentation">
	<div class="modal" onclick={(e) => e.stopPropagation()} role="dialog">
		<header>
			<strong>Pick a folder on the server</strong>
			<button onclick={onclose}>✕</button>
		</header>
		{#if loading}
			<p>Loading…</p>
		{:else if error}
			<p class="err">{error}</p>
		{:else if current}
			<div class="crumbs">
				{#if current.parent}
					<button onclick={() => go(current!.parent!)}>← up</button>
				{/if}
				<code>{current.path}</code>
			</div>
			<ul>
				{#each current.entries.filter(e => e.is_dir) as e}
					<li>
						<button class="entry" onclick={() => go(`${current!.path === '/' ? '' : current!.path}/${e.name}`)}>
							📁 {e.name}
						</button>
					</li>
				{/each}
			</ul>
			<footer>
				<label><input type="checkbox" bind:checked={recursive} /> recursive</label>
				<button class="primary" onclick={select}>Select this folder</button>
			</footer>
		{/if}
	</div>
</div>

<style>
	.backdrop { position: fixed; inset: 0; background: rgba(0,0,0,0.4); display: flex; align-items: center; justify-content: center; }
	.modal { background: white; width: 90%; max-width: 600px; max-height: 80vh; display: flex; flex-direction: column; border-radius: 6px; padding: 1rem; }
	header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem; }
	.crumbs { display: flex; align-items: center; gap: 0.5rem; font-size: 0.85rem; padding: 0.4rem 0; border-bottom: 1px solid #eee; }
	.crumbs code { font-family: ui-monospace, monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
	ul { list-style: none; padding: 0; margin: 0; overflow-y: auto; flex: 1; }
	li { padding: 0.1rem 0; }
	.entry { display: block; width: 100%; text-align: left; background: none; border: none; padding: 0.3rem 0.4rem; cursor: pointer; font-size: 0.9rem; }
	.entry:hover { background: #f3f3f3; }
	footer { display: flex; justify-content: space-between; align-items: center; padding-top: 0.6rem; border-top: 1px solid #eee; }
	button { cursor: pointer; }
	button.primary { background: #222; color: white; border: none; padding: 0.5rem 0.9rem; border-radius: 4px; }
	.err { color: #c33; }
</style>
