<template>
  <div v-if="show" class="modal-overlay" @click="$emit('close')">
    <div class="modal" @click.stop>
      <div class="modal-header">
        <h3>Add Friend</h3>
        <button @click="$emit('close')" class="btn-close">&times;</button>
      </div>
      <div class="modal-body">
        <form @submit.prevent="handleSubmit">
          <div class="form-group">
            <label for="peerId">Peer ID *</label>
            <input
              id="peerId"
              v-model="form.peerId"
              type="text"
              placeholder="Enter peer ID"
              required
            />
          </div>
          <div class="form-group">
            <label for="publicKey">Public Key *</label>
            <input
              id="publicKey"
              v-model="form.publicKey"
              type="text"
              placeholder="Enter E2E public key"
              required
            />
          </div>
          <div class="form-group">
            <label for="nickname">Nickname (optional)</label>
            <input
              id="nickname"
              v-model="form.nickname"
              type="text"
              placeholder="Enter nickname"
            />
          </div>
          <div v-if="error" class="error-message">{{ error }}</div>
          <div class="modal-actions">
            <button type="button" @click="$emit('close')" class="btn-secondary">
              Cancel
            </button>
            <button type="submit" class="btn-primary" :disabled="submitting">
              {{ submitting ? 'Adding...' : 'Add Friend' }}
            </button>
          </div>
        </form>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useFriendsStore } from '@/stores/friends'

const props = defineProps<{
  show: boolean
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
watch(() => props.show, (show) => {
  if (!show) {
    form.value = { peerId: '', publicKey: '', nickname: '' }
    error.value = null
  }
})
</script>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal {
  background: white;
  border-radius: 8px;
  width: 90%;
  max-width: 500px;
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
}

.modal-header {
  padding: 16px 20px;
  border-bottom: 1px solid #e0e0e0;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.modal-header h3 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
}

.btn-close {
  background: none;
  border: none;
  font-size: 28px;
  color: #6c757d;
  cursor: pointer;
  padding: 0;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
}

.btn-close:hover {
  background: #f0f0f0;
}

.modal-body {
  padding: 20px;
}

.form-group {
  margin-bottom: 16px;
}

.form-group label {
  display: block;
  margin-bottom: 6px;
  font-weight: 500;
  font-size: 14px;
  color: #212529;
}

.form-group input {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid #e0e0e0;
  border-radius: 4px;
  font-size: 14px;
  box-sizing: border-box;
}

.form-group input:focus {
  outline: none;
  border-color: #007bff;
}

.error-message {
  color: #dc3545;
  font-size: 14px;
  margin-bottom: 16px;
  padding: 8px 12px;
  background: #f8d7da;
  border-radius: 4px;
}

.modal-actions {
  display: flex;
  gap: 12px;
  justify-content: flex-end;
}

.btn-primary,
.btn-secondary {
  padding: 10px 20px;
  border: none;
  border-radius: 4px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s;
}

.btn-primary {
  background: #007bff;
  color: white;
}

.btn-primary:hover:not(:disabled) {
  background: #0056b3;
}

.btn-primary:disabled {
  background: #6c757d;
  cursor: not-allowed;
}

.btn-secondary {
  background: #6c757d;
  color: white;
}

.btn-secondary:hover {
  background: #545b62;
}
</style>
