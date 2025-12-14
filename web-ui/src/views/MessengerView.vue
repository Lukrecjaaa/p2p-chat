<template>
  <div class="messenger">
    <ChatList
      @select-conversation="handleSelectConversation"
      @toggle-add-friend="showAddFriendModal = !showAddFriendModal"
      @toggle-info="showInfoPanel = !showInfoPanel"
      @toggle-status="showStatusPanel = !showStatusPanel"
    />
    <ChatWindow />
    <InfoPanel
      :visible="showInfoPanel"
      @close="showInfoPanel = false"
    />
    <StatusPanel
      :visible="showStatusPanel"
      @close="showStatusPanel = false"
    />
    <AddFriendModal
      :visible="showAddFriendModal"
      @close="showAddFriendModal = false"
      @success="handleFriendAdded"
    />
  </div>
</template>

<script setup lang="ts">
/**
 * @file MessengerView.vue
 * @brief The main view component for the messenger application.
 * It orchestrates various components like ChatList, ChatWindow, InfoPanel, StatusPanel, and AddFriendModal.
 * It also handles WebSocket communication for real-time updates and manages global application state through Pinia stores.
 */
import { ref, onMounted, onUnmounted } from 'vue'
import ChatList from '@/components/ChatList.vue'
import ChatWindow from '@/components/ChatWindow.vue'
import InfoPanel from '@/components/InfoPanel.vue'
import StatusPanel from '@/components/StatusPanel.vue'
import AddFriendModal from '@/components/AddFriendModal.vue'
import { useIdentityStore } from '@/stores/identity'
import { useFriendsStore } from '@/stores/friends'
import { useConversationsStore } from '@/stores/conversations'
import { wsManager } from '@/api/websocket'
import type { WebSocketMessage } from '@/api/types'

/**
 * Reactive state to control the visibility of the Add Friend modal.
 * @type {Ref<boolean>}
 */
const showAddFriendModal = ref(false)
/**
 * Reactive state to control the visibility of the Info Panel.
 * @type {Ref<boolean>}
 */
const showInfoPanel = ref(false)
/**
 * Reactive state to control the visibility of the Status Panel.
 * @type {Ref<boolean>}
 */
const showStatusPanel = ref(false)

/**
 * Pinia store for managing user identity.
 * @type {IdentityStore}
 */
const identityStore = useIdentityStore()
/**
 * Pinia store for managing friends and their statuses.
 * @type {FriendsStore}
 */
const friendsStore = useFriendsStore()
/**
 * Pinia store for managing conversations and messages.
 * @type {ConversationsStore}
 */
const conversationsStore = useConversationsStore()

/**
 * Handles the selection of a conversation from the ChatList.
 * Sets the active conversation in the conversations store.
 * @param {string} peerId - The ID of the peer whose conversation is selected.
 */
function handleSelectConversation(peerId: string) {
  conversationsStore.setActiveConversation(peerId)
}

/**
 * Handles the event when a new friend is successfully added.
 * Refreshes the list of friends and conversations.
 */
async function handleFriendAdded() {
  await friendsStore.fetchFriends()
  await conversationsStore.fetchConversations()
}

/**
 * Processes incoming WebSocket messages.
 * Dispatches actions to appropriate stores based on message type (new message, peer status, delivery updates).
 * @param {WebSocketMessage} msg - The received WebSocket message.
 */
function handleWebSocketMessage(msg: WebSocketMessage) {
  console.log('[WebSocket] Received message:', msg)
  if (msg.type === 'new_message') {
    // Insert the message directly - WebSocket now includes full content
    const fullMessage = {
      id: msg.id,
      sender: msg.sender,
      recipient: msg.recipient,
      content: msg.content,
      timestamp: msg.timestamp,
      nonce: msg.nonce,
      delivery_status: msg.delivery_status,
    }

    conversationsStore.insertMessage(fullMessage)
    conversationsStore.updateConversationLastMessage(fullMessage)
  } else if (msg.type === 'peer_connected') {
    friendsStore.updatePeerOnlineStatus(msg.peer_id, true)
    conversationsStore.updatePeerOnlineStatus(msg.peer_id, true)
  } else if (msg.type === 'peer_disconnected') {
    friendsStore.updatePeerOnlineStatus(msg.peer_id, false)
    conversationsStore.updatePeerOnlineStatus(msg.peer_id, false)
  } else if (msg.type === 'delivery_status_update') {
    console.log('[WebSocket] Updating delivery status:', msg.message_id, msg.new_status)
    conversationsStore.updateMessageDeliveryStatus(msg.message_id, msg.new_status)
  }
}

/**
 * Stores the unsubscribe function for WebSocket messages.
 * @type {(() => void) | null}
 */
let unsubscribe: (() => void) | null = null

/**
 * Lifecycle hook that runs after the component is mounted.
 * Fetches initial data, starts periodic online peers update, and connects to WebSocket.
 */
onMounted(async () => {
  // Load initial data
  await identityStore.fetchIdentity()
  await friendsStore.fetchFriends()
  await conversationsStore.fetchConversations()

  // Start periodic online peers update
  friendsStore.updateOnlinePeers()
  const onlinePeersInterval = setInterval(() => {
    friendsStore.updateOnlinePeers()
  }, 30000) // Every 30 seconds

  // Connect WebSocket and subscribe to messages
  wsManager.connect()
  unsubscribe = wsManager.onMessage(handleWebSocketMessage)

  // Store interval ID for cleanup
  ;(onMounted as any).onlinePeersInterval = onlinePeersInterval
})

/**
 * Lifecycle hook that runs before the component is unmounted.
 * Disconnects from WebSocket and clears any ongoing intervals.
 */
onUnmounted(() => {
  wsManager.disconnect()
  if (unsubscribe) {
    unsubscribe()
  }
  if ((onMounted as any).onlinePeersInterval) {
    clearInterval((onMounted as any).onlinePeersInterval)
  }
})
</script>

<style scoped>
.messenger {
  display: flex;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  position: relative;
}
</style>
