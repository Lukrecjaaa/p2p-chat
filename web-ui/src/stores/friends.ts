import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Friend } from '@/api/types'
import { listFriends, addFriend as apiAddFriend, getOnlinePeers } from '@/api/client'

export const useFriendsStore = defineStore('friends', () => {
  const friends = ref<Friend[]>([])
  const onlinePeers = ref<string[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  const friendsMap = computed(() => {
    const map = new Map<string, Friend>()
    friends.value.forEach(f => map.set(f.peer_id, f))
    return map
  })

  async function fetchFriends() {
    loading.value = true
    error.value = null
    try {
      friends.value = await listFriends()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch friends'
      throw e
    } finally {
      loading.value = false
    }
  }

  async function addFriend(peerId: string, publicKey: string, nickname?: string) {
    try {
      await apiAddFriend(peerId, publicKey, nickname)
      await fetchFriends()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to add friend'
      throw e
    }
  }

  async function updateOnlinePeers() {
    try {
      onlinePeers.value = await getOnlinePeers()
      // Update online status in friends list
      friends.value.forEach(friend => {
        friend.online = onlinePeers.value.includes(friend.peer_id)
      })
    } catch (e) {
      // Silently fail for online status updates
      console.error('Failed to update online peers:', e)
    }
  }

  function getFriend(peerId: string): Friend | undefined {
    return friendsMap.value.get(peerId)
  }

  function updatePeerOnlineStatus(peerId: string, online: boolean) {
    const friend = friendsMap.value.get(peerId)
    if (friend) {
      friend.online = online
    }
    if (online && !onlinePeers.value.includes(peerId)) {
      onlinePeers.value.push(peerId)
    } else if (!online) {
      onlinePeers.value = onlinePeers.value.filter((p) => p !== peerId)
    }
  }

  return {
    friends,
    onlinePeers,
    loading,
    error,
    friendsMap,
    fetchFriends,
    addFriend,
    updateOnlinePeers,
    getFriend,
    updatePeerOnlineStatus
  }
})
