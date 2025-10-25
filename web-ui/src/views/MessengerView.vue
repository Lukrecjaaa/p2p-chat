<template>
  <div class="messenger">
    <ChatList
      @select-conversation="handleSelectConversation"
      @open-add-friend="showAddFriendModal = true"
    />
    <ChatWindow />
    <AddFriendModal
      :show="showAddFriendModal"
      @close="showAddFriendModal = false"
      @success="handleFriendAdded"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import ChatList from '@/components/ChatList.vue'
import ChatWindow from '@/components/ChatWindow.vue'
import AddFriendModal from '@/components/AddFriendModal.vue'
import { useIdentityStore } from '@/stores/identity'
import { useFriendsStore } from '@/stores/friends'
import { useConversationsStore } from '@/stores/conversations'
import { wsManager } from '@/api/websocket'
import type { WebSocketMessage } from '@/api/types'

const showAddFriendModal = ref(false)

const identityStore = useIdentityStore()
const friendsStore = useFriendsStore()
const conversationsStore = useConversationsStore()

function handleSelectConversation(peerId: string) {
  conversationsStore.setActiveConversation(peerId)
}

async function handleFriendAdded() {
  await friendsStore.fetchFriends()
  await conversationsStore.fetchConversations()
}

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

let unsubscribe: (() => void) | null = null

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
}
</style>
