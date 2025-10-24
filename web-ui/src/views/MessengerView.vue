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
  if (msg.type === 'new_message') {
    // Fetch the full message content from the API
    const peerId = msg.sender === identityStore.identity?.peer_id ? msg.recipient : msg.sender
    conversationsStore.fetchMessages(peerId)
    conversationsStore.fetchConversations()
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
