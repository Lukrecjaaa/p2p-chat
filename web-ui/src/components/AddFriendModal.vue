<template>
  <DraggableWindow
    v-if="visible"
    :initial-x="300"
    :initial-y="100"
    :visible="visible"
    @close="$emit('close')"
  >
    <template #title>
      <img src="/friends-folder.ico" alt="" class="title-icon" />
      <span>Add Friend</span>
    </template>
    <form @submit.prevent="handleSubmit">
      <fieldset>
        <div class="field-row">
          <label for="peerId">Peer ID:</label>
          <input
            id="peerId"
            v-model="form.peerId"
            type="text"
            placeholder="Enter peer ID"
            required
          />
        </div>
        <div class="field-row">
          <label for="publicKey">Public Key:</label>
          <input
            id="publicKey"
            v-model="form.publicKey"
            type="text"
            placeholder="Enter E2E public key"
            required
          />
        </div>
        <div class="field-row">
          <label for="nickname">Nickname:</label>
          <input
            id="nickname"
            v-model="form.nickname"
            type="text"
            placeholder="Enter nickname (optional)"
          />
        </div>
      </fieldset>
      <div v-if="error" class="error-message">
        <img src="/status-offline.ico" alt="" class="error-icon" />
        {{ error }}
      </div>
      <div class="modal-actions">
        <button type="button" @click="$emit('close')">
          Cancel
        </button>
        <button type="submit" :disabled="submitting">
          {{ submitting ? 'Adding...' : 'Add Friend' }}
        </button>
      </div>
    </form>
  </DraggableWindow>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useFriendsStore } from '@/stores/friends'
import DraggableWindow from './DraggableWindow.vue'

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  close: []
  success: []
}>()

const friendsStore = useFriendsStore()

const form = ref({
  peerId: '',
  publicKey: '',
  nickname: ''
})

const submitting = ref(false)
const error = ref<string | null>(null)

async function handleSubmit() {
  if (submitting.value) return

  submitting.value = true
  error.value = null

  try {
    await friendsStore.addFriend(
      form.value.peerId,
      form.value.publicKey,
      form.value.nickname || undefined
    )
    emit('success')
    emit('close')
    // Reset form
    form.value = { peerId: '', publicKey: '', nickname: '' }
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to add friend'
  } finally {
    submitting.value = false
  }
}

// Reset form when modal is closed
watch(() => props.visible, (show) => {
  if (!show) {
    form.value = { peerId: '', publicKey: '', nickname: '' }
    error.value = null
  }
})
</script>

<style scoped>
fieldset {
  border: 1px solid #ccc;
  padding: 16px;
  margin-bottom: 16px;
}

.field-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 12px;
}

.field-row:last-child {
  margin-bottom: 0;
}

.field-row label {
  font-size: 12px;
  font-weight: 600;
  color: #333;
}

.field-row input {
  width: 100%;
  box-sizing: border-box;
}

.error-message {
  color: #dc3545;
  font-size: 13px;
  margin-bottom: 16px;
  padding: 10px 12px;
  background: #f8d7da;
  border: 1px solid #f5c6cb;
  display: flex;
  align-items: center;
  gap: 8px;
}

.error-icon {
  width: 48px;
  height: 48px;
  image-rendering: crisp-edges;
  flex-shrink: 0;
}

.modal-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.title-icon {
  width: 16px;
  height: 16px;
  vertical-align: middle;
  margin-right: 4px;
  image-rendering: crisp-edges;
}
</style>
