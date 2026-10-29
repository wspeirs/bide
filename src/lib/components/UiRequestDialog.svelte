<script lang="ts">
  import { projectsStore } from '../stores/projects.svelte';
  import type { ProjectState, AskQuestion } from '../types';
  import { X, HelpCircle } from 'lucide-svelte';

  interface Props {
    project: ProjectState;
  }

  let { project }: Props = $props();

  let req = $derived(project.pendingUiRequest);

  let inputValue = $state('');
  let picks = $state<Record<string, string[]>>({});
  let customs = $state<Record<string, string>>({});

  $effect(() => {
    inputValue = req?.defaultValue || '';
    picks = {};
    customs = {};
  });

  // single-select questions need an answer; multi-select may be empty
  let canSubmitAsk = $derived(
    !!req?.questions?.every((q) => q.multi || picks[q.id]?.length || customs[q.id]?.trim()),
  );

  function cancel() {
    if (!req) return;
    projectsStore.respondUi(project.id, { cancelled: true });
  }

  function handleSelect(option: string) {
    if (!req) return;
    projectsStore.respondUi(project.id, { value: option });
  }

  function handleConfirm(val: boolean) {
    if (!req) return;
    projectsStore.respondUi(project.id, { confirmed: val });
  }

  function handleSubmitInput() {
    if (!req) return;
    projectsStore.respondUi(project.id, { value: inputValue });
  }

  function togglePick(q: AskQuestion, label: string) {
    const cur = picks[q.id] ?? [];
    if (q.multi) {
      picks[q.id] = cur.includes(label) ? cur.filter((l) => l !== label) : [...cur, label];
    } else {
      picks[q.id] = cur.includes(label) ? [] : [label];
      customs[q.id] = '';
    }
  }

  function setCustom(q: AskQuestion, v: string) {
    customs[q.id] = v;
    if (!q.multi && v.trim()) picks[q.id] = [];
  }

  function handleSubmitAsk() {
    if (!req?.questions || !canSubmitAsk) return;
    const answers = req.questions.map((q) => {
      const customInput = customs[q.id]?.trim();
      return { id: q.id, selectedOptions: picks[q.id] ?? [], ...(customInput ? { customInput } : {}) };
    });
    projectsStore.respondUi(project.id, { answers });
  }
</script>

{#if req}
  <div
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs select-none"
  >
    <button
      type="button"
      class="fixed inset-0 cursor-default bg-transparent border-none p-0 outline-none"
      onclick={cancel}
      aria-label="Cancel dialog"
    ></button>
    <div class="relative w-full max-w-lg flex flex-col bg-[#1e1f22] border border-[#393b40] rounded shadow-2xl overflow-hidden z-10 text-xs font-sans">
      <!-- Header -->
      <div class="flex items-center justify-between px-4 py-2.5 bg-[#252629] border-b border-[#2b2d30] shrink-0">
        <div class="flex items-center gap-2">
          <HelpCircle class="w-4 h-4 text-[#3574f0]" />
          <span class="font-semibold text-[#dfe1e5]">{req.title || 'Input Required'}</span>
        </div>
        <button
          type="button"
          onclick={cancel}
          class="p-1 rounded text-[#6f737a] hover:text-[#dfe1e5] hover:bg-[#35373c] transition-colors"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Body -->
      <div class="p-4 space-y-3 max-h-[70vh] overflow-y-auto">
        {#if req.message}
          <div class="text-xs text-[#bcbec4] leading-relaxed whitespace-pre-wrap">{req.message}</div>
        {/if}

        {#if req.method === 'select' && req.options}
          <div class="flex flex-col gap-1.5 pt-1">
            {#each req.options as opt (opt)}
              <button
                type="button"
                onclick={() => handleSelect(opt)}
                class="w-full text-left px-3 py-2 rounded bg-[#252629] border border-[#393b40] hover:bg-[#2b2d30] hover:border-[#3574f0] text-[#dfe1e5] transition-colors text-xs font-medium cursor-pointer"
              >
                {opt}
              </button>
            {/each}
          </div>
        {:else if req.method === 'confirm'}
          <div class="flex items-center justify-end gap-2 pt-2">
            <button
              type="button"
              onclick={() => handleConfirm(false)}
              class="px-4 py-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors"
            >
              No
            </button>
            <button
              type="button"
              onclick={() => handleConfirm(true)}
              class="px-4 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors"
            >
              Yes
            </button>
          </div>
        {:else if req.method === 'input'}
          <form
            onsubmit={(e) => {
              e.preventDefault();
              handleSubmitInput();
            }}
            class="space-y-3"
          >
            <input
              type="text"
              bind:value={inputValue}
              class="w-full px-3 py-2 rounded bg-[#1e1f22] border border-[#393b40] focus:border-[#3574f0] text-[#dfe1e5] text-xs font-mono outline-none"
              placeholder="Enter value…"
            />
            <div class="flex items-center justify-end gap-2">
              <button
                type="button"
                onclick={cancel}
                class="px-3 py-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors"
              >
                Cancel
              </button>
              <button
                type="submit"
                class="px-4 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors"
              >
                OK
              </button>
            </div>
          </form>
        {:else if req.method === 'ask' && req.questions}
          <form
            onsubmit={(e) => {
              e.preventDefault();
              handleSubmitAsk();
            }}
            class="space-y-4"
          >
            {#each req.questions as q (q.id)}
              <div class="space-y-1.5">
                {#if q.header}
                  <div class="text-[11px] uppercase tracking-wider text-[#6f737a]">{q.header}</div>
                {/if}
                <div class="text-[#dfe1e5] font-medium">{q.question}</div>
                {#each q.options as opt (opt.label)}
                  {@const on = picks[q.id]?.includes(opt.label)}
                  <button
                    type="button"
                    onclick={() => togglePick(q, opt.label)}
                    class="w-full text-left px-3 py-2 rounded border text-[#dfe1e5] transition-colors cursor-pointer {on
                      ? 'bg-[#2b3a5c] border-[#3574f0]'
                      : 'bg-[#252629] border-[#393b40] hover:bg-[#2b2d30] hover:border-[#3574f0]'}"
                  >
                    <span class="font-medium">{q.multi ? (on ? '☑' : '☐') : on ? '●' : '○'} {opt.label}</span>
                    {#if opt.description}
                      <span class="block text-[#6f737a] pl-4">{opt.description}</span>
                    {/if}
                  </button>
                {/each}
                <input
                  type="text"
                  value={customs[q.id] ?? ''}
                  oninput={(e) => setCustom(q, e.currentTarget.value)}
                  class="w-full px-3 py-2 rounded bg-[#1e1f22] border border-[#393b40] focus:border-[#3574f0] text-[#dfe1e5] text-xs font-mono outline-none"
                  placeholder="Other (type your own)…"
                />
              </div>
            {/each}
            <div class="flex items-center justify-end gap-2">
              <button
                type="button"
                onclick={cancel}
                class="px-3 py-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={!canSubmitAsk}
                class="px-4 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Submit
              </button>
            </div>
          </form>
        {:else if req.method === 'editor'}
          <div class="space-y-3">
            <textarea
              bind:value={inputValue}
              rows={8}
              class="w-full p-2.5 rounded bg-[#1e1f22] border border-[#393b40] focus:border-[#3574f0] text-[#dfe1e5] text-xs font-mono outline-none resize-y leading-relaxed"
            ></textarea>
            <div class="flex items-center justify-end gap-2">
              <button
                type="button"
                onclick={cancel}
                class="px-3 py-1.5 rounded bg-[#252629] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors"
              >
                Cancel
              </button>
              <button
                type="button"
                onclick={handleSubmitInput}
                class="px-4 py-1.5 rounded bg-[#3574f0] hover:bg-[#3574f0]/90 text-white font-medium transition-colors"
              >
                OK
              </button>
            </div>
          </div>
        {/if}
      </div>

      <!-- Footer for select method cancel -->
      {#if req.method === 'select'}
        <div class="px-4 py-2 bg-[#252629] border-t border-[#2b2d30] flex justify-end shrink-0">
          <button
            type="button"
            onclick={cancel}
            class="px-3 py-1.5 rounded bg-[#1e1f22] border border-[#393b40] text-[#bcbec4] hover:text-[#dfe1e5] hover:bg-[#2b2d30] transition-colors text-xs"
          >
            Cancel
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}
