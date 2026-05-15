<script lang="ts">
	import FolderList from './FolderList.svelte';
	import BrowseView from './BrowseView.svelte';
	import AddFolderModal from './AddFolderModal.svelte';

	type View = { kind: 'folders' } | { kind: 'browse'; folder: string };

	let view = $state<View>({ kind: 'folders' });
	let showAdd = $state(false);
	let listRef: any = $state();

	async function onAdded() {
		showAdd = false;
		await listRef?.reload();
	}
</script>

<main>
	{#if view.kind === 'folders'}
		<header>
			<h1>dlna-rs</h1>
		</header>
		<FolderList bind:this={listRef} onbrowse={(p) => view = { kind: 'browse', folder: p }} />
		<button class="add" onclick={() => showAdd = true}>+ add folder…</button>
	{:else}
		<BrowseView folder={view.folder} onback={() => view = { kind: 'folders' }} />
	{/if}

	{#if showAdd}
		<AddFolderModal onclose={() => showAdd = false} onadded={onAdded} />
	{/if}
</main>

<style>
	main { max-width: 720px; margin: 2rem auto; font-family: ui-sans-serif, system-ui, sans-serif; padding: 0 1rem; }
	header { display: flex; align-items: baseline; justify-content: space-between; }
	h1 { font-size: 1.25rem; margin: 0 0 1rem; }
	.add { margin-top: 1rem; border: 1px solid #ddd; background: #fafafa; padding: 0.3rem 0.6rem; cursor: pointer; }
	.add:hover { border-color: #aaa; }
</style>
