import type { WebSocketMessage } from './types'

type MessageHandler = (msg: WebSocketMessage) => void

class WebSocketManager {
  private ws: WebSocket | null = null
  private handlers: Set<MessageHandler> = new Set()
  private reconnectTimeout: number | null = null
  private shouldReconnect = true

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

  onMessage(handler: MessageHandler) {
    this.handlers.add(handler)
    return () => this.handlers.delete(handler)
  }
}

export const wsManager = new WebSocketManager()
