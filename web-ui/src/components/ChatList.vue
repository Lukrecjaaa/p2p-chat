/**
 * @file ChatList.vue
 * @brief This component displays a list of chat conversations, user profile information,
 * search functionality, and action buttons. It serves as the main navigation
 * and information panel for the chat application, allowing users to select conversations,
 * view friend status, and access various features.
 */
<template>
  <!--
    @component ChatList
    @description Displays a list of chat conversations, user profile information,
    search functionality, and action buttons. It serves as the main navigation
    and information panel for the chat application.
  -->
  <div class="chat-list window">
    <!-- @element title-bar - The title bar for the chat list window. -->
    <div class="title-bar">
      <div class="title-bar-text">
        <!-- @element titlebar-icon - Icon displayed in the title bar. -->
        <img src="/conversation-select.ico" alt="" class="titlebar-icon" />
        p2p-chat
      </div>
    </div>

    <!-- @element window-body - The main content area of the chat list window. -->
    <div class="window-body has-space">
      <!-- @section User Profile Header -->
      <div class="user-profile-header">
        <!-- @element gradient-canvas - Canvas for rendering background gradient effect. -->
        <canvas id="gradient-canvas-user-header" class="gradient-canvas"></canvas>
        <div class="header-content">
          <!--
            @component FramedAvatar
            @description Displays the user's avatar with a frame.
            @prop {string} name - The name to display or use for avatar generation.
            @prop {string} peerId - The peer ID associated with the user.
            @prop {string} size - The size of the avatar ('large', 'small', etc.).
          -->
          <FramedAvatar :name="identity?.peer_id || 'User'" :peer-id="identity?.peer_id" size="large" />
          <!-- @element user-info - Container for user's name and status. -->
          <div class="user-info">
            <!-- @element user-name - Displays the current user's peer ID or 'User'. -->
            <div class="user-name">{{ identity?.peer_id || 'User' }}</div>
            <!-- @element user-status - Displays the current user's online status. -->
            <div class="user-status">Available</div>
          </div>
        </div>
      </div>

      <!-- @section Search Bar -->
      <div class="search-container">
        <!-- @element contact-search - Input field for searching contacts. -->
        <input
          v-model="searchQuery"
          type="text"
          placeholder="Search contacts..."
          class="contact-search"
        />
      </div>

      <!-- @section Toolbar -->
      <div class="toolbar">
        <!-- @element info-button - Button to toggle user info display. -->
        <button @click="$emit('toggleInfo')" title="Your Info">Info</button>
        <!-- @element status-button - Button to toggle system status display. -->
        <button @click="$emit('toggleStatus')" title="System Status">Status</button>
        <!-- @element add-friend-button - Button to toggle add friend modal. -->
        <button @click="$emit('toggleAddFriend')" title="Add Friend">Add Friend</button>
      </div>

      <!-- @section Conversations Header -->
      <div class="friends-header">
        <!-- @element folder-icon - Icon for the conversations header. -->
        <img src="/friends-folder.ico" alt="" class="folder-icon" />
        <span>Conversations</span>
      </div>

      <!-- @section Conversation List -->
      <!-- @element loading-indicator - Displays when conversations are loading. -->
      <div v-if="loading" class="loading">Loading...</div>
      <!-- @element error-display - Displays error messages if conversation fetching fails. -->
      <div v-else-if="error" class="error">{{ error }}</div>
      <!-- @element conversation-table - Table displaying the list of conversations. -->
      <table v-else class="conversation-table">
        <tbody>
          <!--
            @element conversation-row - Individual row representing a conversation.
            @prop {object} conv - The conversation object.
            @prop {string} conv.peer_id - The peer ID of the conversation partner.
            @prop {string | null} conv.nickname - The nickname of the conversation partner.
            @prop {boolean} conv.online - Online status of the conversation partner.
            @prop {object | null} conv.last_message - The last message in the conversation.
            @event click - Emits 'selectConversation' with the peer ID when clicked.
          -->
          <tr
            v-for="conv in filteredConversations"
            :key="conv.peer_id"
            :class="{ highlighted: conv.peer_id === activeConversation }"
            @click="$emit('selectConversation', conv.peer_id)"
          >
            <!-- @element avatar-cell - Cell containing the conversation partner's avatar and online status. -->
            <td class="avatar-cell">
              <FramedAvatar :name="conv.nickname || conv.peer_id" :peer-id="conv.peer_id" size="small" />
              <!-- @element status-icon - Displays online/offline status icon for the conversation partner. -->
              <img
                class="status-icon"
                :src="conv.online ? '/status-online.ico' : '/status-offline.ico'"
                :alt="conv.online ? 'Online' : 'Offline'"
              />
            </td>
            <!-- @element info-cell - Cell containing the conversation partner's name and last message. -->
            <td class="info-cell">
              <!-- @element peer-name - Displays the nickname or truncated peer ID of the conversation partner. -->
              <div class="peer-name">{{ conv.nickname || truncatePeerId(conv.peer_id) }}</div>
              <!-- @element last-message - Displays the content of the last message or 'No messages yet'. -->
              <div class="last-message">{{ conv.last_message?.content || 'No messages yet' }}</div>
            </td>
            <!-- @element time-cell - Cell containing the timestamp of the last message. -->
            <td class="time-cell">
              <!-- @element timestamp - Displays the formatted timestamp of the last message. -->
              <span v-if="conv.last_message" class="timestamp">
                {{ formatTimestamp(conv.last_message.timestamp) }}
              </span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { storeToRefs } from 'pinia'
import { useConversationsStore } from '@/stores/conversations'
import { useIdentityStore } from '@/stores/identity'
import FramedAvatar from './FramedAvatar.vue'
import gradientGL from 'gradient-gl'

/**
 * @emits
 * @event selectConversation - Emitted when a conversation is selected, with the peer ID as payload.
 * @event toggleAddFriend - Emitted when the "Add Friend" button is clicked.
 * @event toggleInfo - Emitted when the "Info" button is clicked.
 * @event toggleStatus - Emitted when the "Status" button is clicked.
 */
defineEmits<{
  selectConversation: [peerId: string]
  toggleAddFriend: []
  toggleInfo: []
  toggleStatus: []
}>()

/**
 * Pinia store for managing conversations.
 * @type {ReturnType<typeof useConversationsStore>}
 */
const conversationsStore = useConversationsStore()
/**
 * Pinia store for managing the user's identity.
 * @type {ReturnType<typeof useIdentityStore>}
 */
const identityStore = useIdentityStore()
/**
 * Destructured reactive references from the conversations store for easy access.
 * @property {Ref<Array>} sortedConversations - Sorted list of conversations.
 * @property {Ref<string | null>} activeConversation - The currently active conversation's peer ID.
 * @property {Ref<boolean>} loading - Loading state for conversations.
 * @property {Ref<string | null>} error - Error message for conversations.
 */
const { sortedConversations, activeConversation, loading, error } = storeToRefs(conversationsStore)
/**
 * Destructured reactive reference for the user's identity from the identity store.
 * @property {Ref<Identity | null>} identity - The current user's identity.
 */
const { identity } = storeToRefs(identityStore)

/**
 * Reactive state for the search query input.
 * @type {Ref<string>}
 */
const searchQuery = ref('')
/**
 * Reactive reference to the canvas element used for the gradient background.
 * @type {Ref<HTMLCanvasElement | null>}
 */
const gradientCanvas = ref<HTMLCanvasElement | null>(null)
/**
 * Instance of the gradientGL library.
 * @type {any}
 */
let gradientInstance: any = null

/**
 * Computed property that filters the list of conversations based on the search query.
 * Searches across nickname, peer ID, and last message content.
 * @computed
 * @returns {Array} The filtered list of conversations.
 */
const filteredConversations = computed(() => {
  if (!searchQuery.value.trim()) {
    return sortedConversations.value
  }

  const query = searchQuery.value.toLowerCase()
  return sortedConversations.value.filter(conv => {
    const nickname = conv.nickname?.toLowerCase() || ''
    const peerId = conv.peer_id.toLowerCase()
    const lastMessage = conv.last_message?.content?.toLowerCase() || ''

    return nickname.includes(query) ||
           peerId.includes(query) ||
           lastMessage.includes(query)
  })
})

/**
 * Truncates a peer ID for display purposes, showing the beginning and end with an ellipsis in between.
 * @function truncatePeerId
 * @param {string} peerId - The full peer ID to truncate.
 * @returns {string} The truncated peer ID.
 */
function truncatePeerId(peerId: string): string {
  if (peerId.length <= 12) return peerId
  return peerId.substring(0, 8) + '...' + peerId.substring(peerId.length - 4)
}

/**
 * Formats a Unix timestamp into a human-readable string.
 * Displays time for messages within 24 hours, weekday for messages within 7 days,
 * and date for older messages.
 * @function formatTimestamp
 * @param {number} timestamp - The Unix timestamp in milliseconds.
 * @returns {string} The formatted date/time string.
 */
function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp)
  const now = new Date()
  const diff = now.getTime() - date.getTime()
  const hours = diff / (1000 * 60 * 60)

  if (hours < 24) {
    return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' })
  } else if (hours < 168) { // 7 days
    return date.toLocaleDateString('en-US', { weekday: 'short' })
  } else {
    return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  }
}

/**
 * Lifecycle hook that runs after the component is mounted.
 * Initializes the gradient background using gradientGL.
 * @function onMounted
 * @async
 */
onMounted(async () => {
  // gradient-gl uses a string ID to reference gradients
  // The first param is a gradient preset ID, second is a CSS selector
  gradientInstance = await gradientGL('b1.365e', '#gradient-canvas-user-header')
})

/**
 * Lifecycle hook that runs before the component is unmounted.
 * Cleans up the gradientGL instance if necessary.
 * @function onUnmounted
 */
onUnmounted(() => {
  // Clean up if needed
  gradientInstance = null
})
</script>

<style scoped>
.chat-list {
  width: 320px;
  display: flex;
  flex-direction: column;
  height: 100vh;
  border-radius: 0;
}

/* User Profile Header */
.user-profile-header {
  position: relative;
  overflow: hidden;
  border-bottom: 1px solid #d0d0d0;
  height: 160px;
}

.gradient-canvas {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 1;
}

.header-content {
  position: relative;
  z-index: 2;
  padding: 16px;
  display: flex;
  gap: 12px;
  align-items: center;
  height: 100%;
}

.header-content::before {
  content: '✦ ✧ ✦ ✧ ✦ ✧ ✦ ✧ ✦ ✧ ✦ ✧';
  position: absolute;
  top: 50%;
  left: -100%;
  width: 300%;
  transform: translateY(-50%);
  font-size: 16px;
  color: rgba(255, 255, 255, 0.6);
  white-space: nowrap;
  animation: stars-scroll 20s linear infinite;
  pointer-events: none;
  z-index: 3;
  letter-spacing: 40px;
}

.header-content::after {
  content: '✦ ✧ ✦ ✧ ✦ ✧ ✦ ✧ ✦ ✧';
  position: absolute;
  top: 25%;
  left: -100%;
  width: 300%;
  transform: translateY(-50%);
  font-size: 12px;
  color: rgba(255, 255, 255, 0.4);
  white-space: nowrap;
  animation: stars-scroll 15s linear infinite reverse;
  pointer-events: none;
  z-index: 3;
  letter-spacing: 60px;
}

.user-info {
  position: relative;
  z-index: 4;
}

@keyframes stars-scroll {
  0% {
    left: -100%;
  }
  100% {
    left: 100%;
  }
}

.user-info {
  flex: 1;
  min-width: 0;
}

.user-name {
  font-size: 16px;
  font-weight: 600;
  color: #000;
  margin-bottom: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.user-status {
  font-size: 12px;
  color: #28a745;
}

/* Search Bar */
.search-container {
  padding: 8px;
  background: #f5f5f5;
  border-bottom: 1px solid #d0d0d0;
}

.contact-search {
  width: 100%;
  padding: 4px 28px 4px 8px;
  border: 1px solid #ccc;
  border-radius: 3px;
  font-size: 12px;
  background-image: url('/search-icon.ico');
  background-repeat: no-repeat;
  background-position: right 6px center;
  background-size: 16px 16px;
  image-rendering: crisp-edges;
}

.toolbar {
  padding: 8px;
  border-bottom: 1px solid #ccc;
  display: flex;
  gap: 4px;
  background: #f0f0f0;
}

.friends-header {
  padding: 8px 12px;
  background: #e8e8e8;
  border-bottom: 1px solid #d0d0d0;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
  color: #333;
}

.folder-icon {
  width: 16px;
  height: 16px;
  image-rendering: crisp-edges;
}

.window-body {
  flex: 1;
  overflow-y: auto;
  padding: 0;
  background: white;
}

.conversation-table {
  width: 100%;
  border-collapse: collapse;
}

.loading,
.error {
  padding: 16px;
  text-align: center;
}

.error {
  color: #dc3545;
}

.conversation-table {
  width: 100%;
  border-collapse: collapse;
}

.conversation-table tbody tr {
  cursor: pointer;
  transition: background 0.15s;
  border-bottom: 1px solid #e0e0e0;
}

.conversation-table tbody tr:hover {
  background: #f0f0f0;
}

.conversation-table tbody tr.highlighted {
  background: #e3f2fd;
}

.avatar-cell {
  width: 60px;
  padding: 8px;
  position: relative;
  text-align: center;
}

.status-icon {
  position: absolute;
  bottom: 8px;
  right: 8px;
  width: 16px;
  height: 16px;
  image-rendering: crisp-edges;
  z-index: 3;
}

.info-cell {
  padding: 8px 8px 8px 0;
  max-width: 180px;
}

.peer-name {
  font-weight: 600;
  font-size: 14px;
  color: #212529;
  margin-bottom: 2px;
}

.last-message {
  font-size: 12px;
  color: #6c757d;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.time-cell {
  padding: 8px;
  text-align: right;
  vertical-align: top;
  width: 70px;
  min-width: 70px;
}

.timestamp {
  font-size: 11px;
  color: #6c757d;
  white-space: nowrap;
}

.titlebar-icon {
  width: 16px;
  height: 16px;
  vertical-align: middle;
  margin-right: 4px;
  image-rendering: crisp-edges;
}
</style>
