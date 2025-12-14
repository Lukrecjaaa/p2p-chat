/**
 * @file identity.ts
 * @brief Pinia store for managing the application's user identity.
 * This store handles fetching and storing the current user's identity information.
 */
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { Identity } from '@/api/types'
import { getMe } from '@/api/client'

/**
 * Pinia store for managing the user's identity.
 * @returns {object} The store's state and actions.
 * @property {Ref<Identity | null>} identity - The current user's identity object.
 * @property {Ref<boolean>} loading - Indicates if the identity is currently being fetched.
 * @property {Ref<string | null>} error - Stores any error message if fetching fails.
 * @property {Function} fetchIdentity - Action to fetch the user's identity from the API.
 */
export const useIdentityStore = defineStore('identity', () => {
  /**
   * The current user's identity object. Null if not yet fetched or on error.
   * @type {Ref<Identity | null>}
   */
  const identity = ref<Identity | null>(null)
  /**
   * Boolean indicating if the identity is currently being loaded.
   * @type {Ref<boolean>}
   */
  const loading = ref(false)
  /**
   * Stores an error message if fetching the identity fails. Null if no error.
   * @type {Ref<string | null>}
   */
  const error = ref<string | null>(null)

  /**
   * Fetches the user's identity from the backend API.
   * Sets loading state, handles success and error cases.
   * @async
   * @throws {Error} If the API call fails.
   */
  async function fetchIdentity() {
    loading.value = true
    error.value = null
    try {
      identity.value = await getMe()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch identity'
      throw e
    } finally {
      loading.value = false
    }
  }

  return {
    identity,
    loading,
    error,
    fetchIdentity
  }
})
