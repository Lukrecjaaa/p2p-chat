/**
 * @file AddFriendModal.vue
 * @brief This component provides a modal interface for users to add new friends
 * by entering their Peer ID, Public Key, and an optional Nickname.
 * It integrates with the `useFriendsStore` for handling friend addition logic
 * and includes error handling and form submission management.
 */
<template>
  <!--
    @component AddFriendModal
    @description A modal component for adding a new friend to the chat application.
    It provides input fields for Peer ID, Public Key, and an optional Nickname,
    and handles the submission of this information.
  -->
  <DraggableWindow
    v-if="visible"
    :initial-x="300"
    :initial-y="100"
    :visible="visible"
    @close="$emit('close')"
  >
    <template #title>
      <!-- @element title-icon - Icon displayed in the modal title -->
      <img src="/friends-folder.ico" alt="" class="title-icon" />
      <!-- @element title-text - Text displayed in the modal title -->
      <span>Add Friend</span>
    </template>
    <!-- @element add-friend-form - Form for submitting friend details -->
    <form @submit.prevent="handleSubmit">
      <fieldset>
        <!-- @element peer-id-field - Input field for the friend's Peer ID -->
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
        <!-- @element public-key-field - Input field for the friend's Public Key -->
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
        <!-- @element nickname-field - Input field for the friend's Nickname (optional) -->
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
      <!-- @element error-message - Displays error messages during friend addition -->
      <div v-if="error" class="error-message">
        <img src="/status-offline.ico" alt="" class="error-icon" />
        {{ error }}
      </div>
      <!-- @element modal-actions - Contains action buttons for the modal -->
      <div class="modal-actions">
        <!-- @element cancel-button - Button to close the modal without adding a friend -->
        <button type="button" @click="$emit('close')">
          Cancel
        </button>
        <!-- @element submit-button - Button to submit the form and add a friend -->
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

/**
 * @props
 * @property {boolean} visible - Controls the visibility of the modal.
 */
const props = defineProps<{
  visible: boolean
}>()

/**
 * @emits
 * @event close - Emitted when the modal is requested to be closed.
 * @event success - Emitted when a friend is successfully added.
 */
const emit = defineEmits<{
  close: []
  success: []
}>()

/**
 * Friends store instance to interact with friend-related state and actions.
 * @type {ReturnType<typeof useFriendsStore>}
 */
const friendsStore = useFriendsStore()

/**
 * Reactive form data for new friend details.
 * @type {Ref<{peerId: string, publicKey: string, nickname: string}>}
 */
const form = ref({
  peerId: '',
  publicKey: '',
  nickname: ''
})

/**
 * Reactive state indicating whether the form is currently being submitted.
 * @type {Ref<boolean>}
 */
const submitting = ref(false)
/**
 * Reactive state holding any error message that occurred during submission.
 * @type {Ref<string | null>}
 */
const error = ref<string | null>(null)

/**
 * Handles the form submission for adding a new friend.
 * It prevents multiple submissions, calls the friend store to add the friend,
 * emits success/close events, and handles error display.
 * @async
 * @function handleSubmit
 * @returns {Promise<void>}
 */
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
    // Reset form after successful submission
    form.value = { peerId: '', publicKey: '', nickname: '' }
  } catch (e) {
    // Catch and display any errors during the friend addition process
    error.value = e instanceof Error ? e.message : 'Failed to add friend'
  } finally {
    // Ensure submitting state is reset regardless of success or failure
    submitting.value = false
  }
}

/**
 * Watches for changes in the `visible` prop. When the modal becomes hidden,
 * it resets the form fields and clears any error messages.
 * @function watch
 * @param {boolean} show - The new value of the `visible` prop.
 */
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
