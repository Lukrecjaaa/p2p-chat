/**
 * @file friends.ts
 * @brief Pinia store for managing the application's friends list and their online statuses.
 * This store handles fetching friends, adding new friends, and updating their online presence.
 */
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Friend } from '@/api/types'
import { listFriends, addFriend as apiAddFriend, getOnlinePeers } from '@/api/client'

/**
 * Pinia store for managing friends.
 * @returns {object} The store's state, getters, and actions.
 * @property {Ref<Friend[]>} friends - A reactive array of friend objects.
 * @property {Ref<string[]>} onlinePeers - A reactive array of peer IDs that are currently online.
 * @property {Ref<boolean>} loading - Indicates if friends data is currently being fetched.
 * @property {Ref<string | null>} error - Stores any error message if an operation fails.
 * @property {ComputedRef<Map<string, Friend>>} friendsMap - A computed property providing a map of friends by peer ID for efficient lookup.
 * @property {Function} fetchFriends - Action to fetch the list of friends from the API.
 * @property {Function} addFriend - Action to add a new friend.
 * @property {Function} updateOnlinePeers - Action to update the online status of friends.
 * @property {Function} getFriend - Getter to retrieve a friend by their peer ID.
 * @property {Function} updatePeerOnlineStatus - Action to update a specific peer's online status.
 */
export const useFriendsStore = defineStore('friends', () => {
  /**
   * Reactive array of friend objects.
   * @type {Ref<Friend[]>}
   */
  const friends = ref<Friend[]>([])
  /**
   * Reactive array of peer IDs that are currently online.
   * @type {Ref<string[]>}
   */
  const onlinePeers = ref<string[]>([])
  /**
   * Boolean indicating if friends data is currently being loaded.
   * @type {Ref<boolean>}
   */
  const loading = ref(false)
  /**
   * Stores an error message if an operation fails. Null if no error.
   * @type {Ref<string | null>}
   */
  const error = ref<string | null>(null)

  /**
   * Computed property that provides a Map of friends, keyed by their peer_id, for efficient lookup.
   * @type {ComputedRef<Map<string, Friend>>}
   */
  const friendsMap = computed(() => {
    const map = new Map<string, Friend>()
    friends.value.forEach(f => map.set(f.peer_id, f))
    return map
  })

  /**
   * Fetches the list of friends from the backend API.
   * Sets loading state, handles success and error cases, and updates online status.
   * @async
   * @throws {Error} If the API call fails.
   */
  async function fetchFriends() {
    loading.value = true
    error.value = null
    try {
      friends.value = await listFriends()
      // After fetching friends, update their online status
      await updateOnlinePeers()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch friends'
      throw e
    } finally {
      loading.value = false
    }
  }

  /**
   * Adds a new friend to the system via the API.
   * After successfully adding, it re-fetches the entire friends list to update the UI.
   * @async
   * @param {string} peerId - The peer ID of the friend to add.
   * @param {string} publicKey - The public key of the friend.
   * @param {string} [nickname] - Optional nickname for the friend.
   * @throws {Error} If the API call to add a friend fails.
   */
  async function addFriend(peerId: string, publicKey: string, nickname?: string) {
    try {
      await apiAddFriend(peerId, publicKey, nickname)
      await fetchFriends() // Re-fetch friends to include the new one
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to add friend'
      throw e
    }
  }

  /**
   * Fetches the list of currently online peers from the API and updates the online status of friends.
   * Errors are silently logged to avoid disrupting the UI.
   * @async
   */
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

  /**
   * Retrieves a friend object by their peer ID.
   * @param {string} peerId - The peer ID of the friend to retrieve.
   * @returns {Friend | undefined} The friend object if found, otherwise undefined.
   */
  function getFriend(peerId: string): Friend | undefined {
    return friendsMap.value.get(peerId)
  }

  /**
   * Updates the online status of a specific peer.
   * This is typically used when receiving WebSocket events for peer connection/disconnection.
   * @param {string} peerId - The peer ID whose online status is to be updated.
   * @param {boolean} online - The new online status (true for online, false for offline).
   */
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
