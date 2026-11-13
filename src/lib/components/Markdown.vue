<script lang="ts">
import MarkdownIt from 'markdown-it';

const md = new MarkdownIt({ html: false, linkify: true });
</script>

<script setup lang="ts">
defineProps<{ source: string }>();

function onClick(e: MouseEvent) {
  if ((e.target as HTMLElement | null)?.closest('a')) e.preventDefault();
}
</script>

<template>
  <div class="md select-text" v-html="md.render(source)" @click="onClick" />
</template>
