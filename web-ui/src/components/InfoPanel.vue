/**
 * @file InfoPanel.vue
 * @brief This component provides a draggable panel that displays the user's
 * identity information, including their Peer ID and Public Key.
 * It offers functionality to copy these details to the clipboard for convenience.
 */
<template>
  <!--
    @component InfoPanel
    @description A draggable window component that displays the user's personal
    information, including their Peer ID and Public Key, with options to copy them.
  -->
  <DraggableWindow
    :initial-x="50"
    :initial-y="50"
    :visible="visible"
    @close="$emit('close')"
  >
    <template #title>
      <!-- @element title-icon - Icon displayed in the panel's title bar. -->
      <img src="/user-info.ico" alt="" class="title-icon" />
      <span>Your Info</span>
    </template>
    <!-- @section identity-info - Displays user's Peer ID and Public Key if available. -->
    <div v-if="identity" class="info-content">
      <!-- @element peer-id-info - Displays the user's Peer ID. -->
      <div class="info-item">
        <label>Peer ID:</label>
        <div class="field-row">
          <!-- @element peer-id-input - Read-only input field for Peer ID. -->
          <input type="text" :value="identity.peer_id" readonly />
          <!-- @element copy-peer-id-button - Button to copy Peer ID to clipboard. -->
          <button @click="copyToClipboard(identity.peer_id)" title="Copy">
            <img src="/copy-icon.ico" alt="Copy" class="copy-icon" />
          </button>
        </div>
      </div>
      <!-- @element public-key-info - Displays the user's Public Key. -->
      <div class="info-item">
        <label>Public Key:</label>
        <div class="field-row">
          <!-- @element public-key-input - Read-only input field for truncated Public Key. -->
          <input type="text" :value="truncateKey(identity.hpke_public_key)" readonly />
          <!-- @element copy-public-key-button - Button to copy Public Key to clipboard. -->
          <button @click="copyToClipboard(identity.hpke_public_key)" title="Copy">
            <img src="/copy-icon.ico" alt="Copy" class="copy-icon" />
          </button>
        </div>
      </div>
    </div>
    <!-- @element info-loading - Displays a loading message if identity information is not yet available. -->
    <div v-else class="info-loading">Loading...</div>
  </DraggableWindow>
</template>

<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { useIdentityStore } from '@/stores/identity'
import DraggableWindow from './DraggableWindow.vue'

/**
 * @props
 * @property {boolean} visible - Controls the visibility of the info panel.
 */
defineProps<{
  visible: boolean
}>()

/**
 * @emits
 * @event close - Emitted when the info panel is requested to be closed.
 */
defineEmits<{
  close: []
}>()

/**
 * Pinia store for managing the user's identity.
 * @type {ReturnType<typeof useIdentityStore>}
 */
const identityStore = useIdentityStore()
/**
 * Destructured reactive reference for the user's identity from the identity store.
 * @property {Ref<Identity | null>} identity - The current user's identity object.
 */
const { identity } = storeToRefs(identityStore)

/**
 * Truncates a long key string for display purposes, showing the beginning and end with an ellipsis.
 * @function truncateKey
 * @param {string} key - The full key string to truncate.
 * @returns {string} The truncated key string.
 */
function truncateKey(key: string): string {
  if (key.length <= 40) return key
  return key.substring(0, 20) + '...' + key.substring(key.length - 20)
}

/**
 * Copies the given text to the user's clipboard.
 * Displays an error to the console if the copy operation fails.
 * @async
 * @function copyToClipboard
 * @param {string} text - The text content to copy.
 * @returns {Promise<void>}
 */
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
