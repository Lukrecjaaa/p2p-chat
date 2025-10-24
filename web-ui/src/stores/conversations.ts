import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Conversation, Message } from '@/api/types'
import { listConversations, getMessages, sendMessage as apiSendMessage } from '@/api/client'
import { useIdentityStore } from './identity'

export const useConversationsStore = defineStore('conversations', () => {
  const conversations = ref<Conversation[]>([])
  const messages = ref<Map<string, Message[]>>(new Map())
  const activeConversation = ref<string | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  const sortedConversations = computed(() => {
    return [...conversations.value].sort((a, b) => {
      const aTime = a.last_message?.timestamp || 0
      const bTime = b.last_message?.timestamp || 0
      return bTime - aTime
    })
  })

  const activeMessages = computed(() => {
    if (!activeConversation.value) return []
    return messages.value.get(activeConversation.value) || []
  })

  async function fetchConversations() {
    loading.value = true
    error.value = null
    try {
      conversations.value = await listConversations()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch conversations'
      throw e
    } finally {
      loading.value = false
    }
  }

  async function fetchMessages(peerId: string) {
    loading.value = true
    error.value = null
    try {
      const msgs = await getMessages(peerId)
      messages.value.set(peerId, msgs)
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch messages'
      throw e
    } finally {
      loading.value = false
    }
  }

  async function sendMessage(peerId: string, content: string) {
    const identityStore = useIdentityStore()
    if (!identityStore.identity) {
      throw new Error('Identity not loaded')
    }

    try {
      const result = await apiSendMessage(peerId, content)

      // Optimistically add message to local state
      const newMessage: Message = {
        id: result.id,
        sender: identityStore.identity.peer_id,
        recipient: peerId,
        content,
        timestamp: Date.now(),
        nonce: 0 // This will be set by backend
      }

      const peerMessages = messages.value.get(peerId) || []
      messages.value.set(peerId, [...peerMessages, newMessage])

      // Update conversation last message
      const conv = conversations.value.find(c => c.peer_id === peerId)
      if (conv) {
        conv.last_message = newMessage
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to send message'
      throw e
    }
  }

  function addMessage(msg: Message) {
    const identityStore = useIdentityStore()
    if (!identityStore.identity) return

    // Determine which peer this message is with
    const peerId = msg.sender === identityStore.identity.peer_id ? msg.recipient : msg.sender

    const peerMessages = messages.value.get(peerId) || []

    // Check if message already exists
    if (peerMessages.some(m => m.id === msg.id)) {
      return
    }

    // Insert message in chronological order (handle past message insertion)
    const newMessages = [...peerMessages, msg].sort((a, b) => a.timestamp - b.timestamp)
    messages.value.set(peerId, newMessages)

    // Update conversation last message if this is newer
    const conv = conversations.value.find(c => c.peer_id === peerId)
    if (conv) {
      if (!conv.last_message || msg.timestamp > conv.last_message.timestamp) {
        conv.last_message = msg
      }
    } else {
      // Create new conversation if it doesn't exist
      conversations.value.push({
        peer_id: peerId,
        nickname: null,
        last_message: msg,
        online: false
      })
    }
  }

  function setActiveConversation(peerId: string | null) {
    activeConversation.value = peerId
  }

  return {
    conversations,
    messages,
    activeConversation,
    loading,
    error,
    sortedConversations,
    activeMessages,
    fetchConversations,
    fetchMessages,
    sendMessage,
    addMessage,
    setActiveConversation
  }
})
