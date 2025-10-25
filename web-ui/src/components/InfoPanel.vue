<template>
  <DraggableWindow
    :initial-x="50"
    :initial-y="50"
    :visible="visible"
    @close="$emit('close')"
  >
    <template #title>
      <img src="/user-info.ico" alt="" class="title-icon" />
      <span>Your Info</span>
    </template>
    <div v-if="identity" class="info-content">
      <div class="info-item">
        <label>Peer ID:</label>
        <div class="field-row">
          <input type="text" :value="identity.peer_id" readonly />
          <button @click="copyToClipboard(identity.peer_id)" title="Copy">
            <img src="/copy-icon.ico" alt="Copy" class="copy-icon" />
          </button>
        </div>
      </div>
      <div class="info-item">
        <label>Public Key:</label>
        <div class="field-row">
          <input type="text" :value="truncateKey(identity.hpke_public_key)" readonly />
          <button @click="copyToClipboard(identity.hpke_public_key)" title="Copy">
            <img src="/copy-icon.ico" alt="Copy" class="copy-icon" />
          </button>
        </div>
      </div>
    </div>
    <div v-else class="info-loading">Loading...</div>
  </DraggableWindow>
</template>

<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { useIdentityStore } from '@/stores/identity'
import DraggableWindow from './DraggableWindow.vue'

defineProps<{
  visible: boolean
}>()

defineEmits<{
  close: []
}>()

const identityStore = useIdentityStore()
const { identity } = storeToRefs(identityStore)

function truncateKey(key: string): string {
  if (key.length <= 40) return key
  return key.substring(0, 20) + '...' + key.substring(key.length - 20)
}

async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text)
  } catch (err) {
    console.error('Failed to copy:', err)
  }
}
</script>

<style scoped>
.info-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.info-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.info-item label {
  font-size: 11px;
  font-weight: 600;
  color: #333;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.field-row {
  display: flex;
  gap: 4px;
  align-items: center;
}

.field-row input {
  flex: 1;
  font-family: 'Courier New', monospace;
  font-size: 11px;
}

.field-row button {
  padding: 4px 8px;
  min-width: auto;
  display: flex;
  align-items: center;
  justify-content: center;
}

.copy-icon {
  width: 16px;
  height: 16px;
  image-rendering: crisp-edges;
}

.info-loading {
  font-size: 12px;
  color: #6c757d;
  text-align: center;
  padding: 16px;
}

.title-icon {
  width: 16px;
  height: 16px;
  vertical-align: middle;
  margin-right: 4px;
  image-rendering: crisp-edges;
}
</style>
