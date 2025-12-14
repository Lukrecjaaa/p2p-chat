/**
 * @file websocket.ts
 * @brief This file provides a WebSocketManager class for handling real-time communication
 * with the backend. It manages the WebSocket connection lifecycle, including
 * connecting, disconnecting, message routing, and automatic reconnection.
 */
import type { WebSocketMessage } from './types'

/**
 * Type definition for a WebSocket message handler.
 * @typedef {function(WebSocketMessage): void} MessageHandler
 */
type MessageHandler = (msg: WebSocketMessage) => void

/**
 * Manages WebSocket connections for real-time communication.
 * Handles connection, disconnection, message parsing, and automatic reconnection.
 * @class WebSocketManager
 */
class WebSocketManager {
  /**
   * The WebSocket instance.
   * @private
   * @type {WebSocket | null}
   */
  private ws: WebSocket | null = null
  /**
   * A set of message handlers to be called when a new WebSocket message is received.
   * @private
   * @type {Set<MessageHandler>}
   */
  private handlers: Set<MessageHandler> = new Set()
  /**
   * The ID of the timeout for WebSocket reconnection attempts.
   * @private
   * @type {number | null}
   */
  private reconnectTimeout: number | null = null
  /**
   * Flag indicating whether the WebSocket should attempt to reconnect on disconnection.
   * @private
   * @type {boolean}
   */
  private shouldReconnect = true

  /**
   * Establishes a WebSocket connection to the server.
   * Handles connection lifecycle, message reception, and automatic reconnection.
   * @public
   */
  connect() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const url = `${protocol}//${window.location.host}/ws`

    this.ws = new WebSocket(url)

    this.ws.onopen = () => {
      console.log('WebSocket connected')
      if (this.reconnectTimeout) {
        clearTimeout(this.reconnectTimeout)
        this.reconnectTimeout = null
      }
    }

    this.ws.onmessage = (event) => {
      try {
        const msg: WebSocketMessage = JSON.parse(event.data)
        this.handlers.forEach((handler) => handler(msg))
      } catch (e) {
        console.error('Failed to parse WebSocket message:', e)
      }
    }

    this.ws.onclose = () => {
      console.log('WebSocket disconnected')
      this.ws = null
      if (this.shouldReconnect) {
        this.reconnectTimeout = window.setTimeout(() => this.connect(), 3000)
      }
    }

    this.ws.onerror = (error) => {
      console.error('WebSocket error:', error)
    }
  }

  /**
   * Disconnects the WebSocket and prevents further reconnection attempts.
   * @public
   */
  disconnect() {
    this.shouldReconnect = false
    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout)
      this.reconnectTimeout = null
    }
    if (this.ws) {
      this.ws.close()
      this.ws = null
    }
  }

  /**
   * Registers a message handler to be called when a new WebSocket message arrives.
   * @param {MessageHandler} handler - The function to call with the received message.
   * @returns {function(): void} A function to unregister the handler.
   * @public
   */
  onMessage(handler: MessageHandler) {
    this.handlers.add(handler)
    return () => this.handlers.delete(handler)
  }
}

/**
 * Singleton instance of the WebSocketManager.
 * @constant {WebSocketManager} wsManager
 */
export const wsManager = new WebSocketManager()
