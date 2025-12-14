//! This module provides a buffer for storing and managing log entries.
//!
//! The `LogBuffer` is used to collect log entries and send them to the UI in
//! batches to avoid overwhelming the UI with too many updates.
use crate::ui::{LogEntry, UIMode};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{interval, Interval};
use tracing::Level;

/// A buffer for storing and managing log entries.
pub struct LogBuffer {
    /// The circular buffer of log entries.
    entries: Arc<Mutex<VecDeque<LogEntry>>>,
    /// The maximum number of entries to store in the buffer.
    max_size: usize,
    /// The sender for sending UI events.
    ui_sender: Arc<Mutex<Option<mpsc::UnboundedSender<crate::ui::UIEvent>>>>,
    /// The current log level to display in the UI.
    current_display_level: Arc<Mutex<Level>>,
    /// The current UI mode.
    current_ui_mode: Arc<Mutex<UIMode>>,
    /// A batch of pending log entries to be sent to the UI.
    pending_batch: Arc<Mutex<Vec<LogEntry>>>,
    /// The timer for sending log batches.
    batch_timer: Arc<Mutex<Option<Interval>>>,
}

impl LogBuffer {
    /// Creates a new `LogBuffer`.
    ///
    /// # Arguments
    ///
    /// * `max_size` - The maximum number of entries to store in the buffer.
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
            ui_sender: Arc::new(Mutex::new(None)),
            current_display_level: Arc::new(Mutex::new(Level::DEBUG)),
            current_ui_mode: Arc::new(Mutex::new(UIMode::Chat)),
            pending_batch: Arc::new(Mutex::new(Vec::new())),
            batch_timer: Arc::new(Mutex::new(None)),
        }
    }

    /// Sets the UI sender.
    ///
    /// This is used to send events to the UI thread.
    pub fn set_ui_sender(&self, sender: mpsc::UnboundedSender<crate::ui::UIEvent>) {
        *self.ui_sender.lock().unwrap() = Some(sender);
    }

    /// Adds a new log entry to the buffer.
    ///
    /// The entry is always stored in the buffer, but it is only sent to the UI
    /// if its level is at or above the current display level.
    pub fn add_entry(&self, entry: LogEntry) {
        // Always store the log entry regardless of display level.
        {
            let mut entries = self.entries.lock().unwrap();
            if entries.len() >= self.max_size {
                entries.pop_front();
            }
            entries.push_back(entry.clone());
        }

        // Notify the UI for logs that meet the display level.
        let should_notify = {
            let current_level = *self.current_display_level.lock().unwrap();
            entry.level <= current_level
        };

        if should_notify {
            // Add to pending batch for async processing.
            self.pending_batch.lock().unwrap().push(entry);

            // Start batch timer if not already running.
            self.start_batch_timer_if_needed();
        }
    }

    /// Sets the current display level for filtering UI notifications.
    pub fn set_display_level(&self, level: Level) {
        *self.current_display_level.lock().unwrap() = level;

        // Trigger a refresh of logs with the new level.
        if let Some(ref sender) = *self.ui_sender.lock().unwrap() {
            let _ = sender.send(crate::ui::UIEvent::RefreshLogs);
        }
    }

    /// Sets the current UI mode to optimize notifications.
    pub fn set_ui_mode(&self, mode: UIMode) {
        *self.current_ui_mode.lock().unwrap() = mode.clone();

        // If switching to log mode, trigger a refresh.
        if matches!(mode, UIMode::Logs { .. }) {
            if let Some(ref sender) = *self.ui_sender.lock().unwrap() {
                let _ = sender.send(crate::ui::UIEvent::RefreshLogs);
            }
        }
    }

    /// Starts the batch timer if it's not already running.
    fn start_batch_timer_if_needed(&self) {
        let mut timer_guard = self.batch_timer.lock().unwrap();
        if timer_guard.is_none() {
            *timer_guard = Some(interval(Duration::from_millis(100))); // Batch every 100ms

            // Clone necessary data for the async task.
            let ui_sender = self.ui_sender.clone();
            let pending_batch = self.pending_batch.clone();
            let batch_timer = self.batch_timer.clone();

            drop(timer_guard); // Release the lock before spawning.

            // Spawn an async task to flush batches.
            tokio::spawn(async move {
                let mut timer = {
                    let mut timer_guard = batch_timer.lock().unwrap();
                    timer_guard.take().unwrap()
                };

                timer.tick().await; // Skip the first immediate tick.

                loop {
                    timer.tick().await;

                    // Check if there are pending entries to flush.
                    let batch = {
                        let mut pending = pending_batch.lock().unwrap();
                        if pending.is_empty() {
                            continue;
                        }
                        let batch = pending.drain(..).collect::<Vec<_>>();
                        batch
                    };

                    // Send batch to UI.
                    if let Some(ref sender) = *ui_sender.lock().unwrap() {
                        if sender.send(crate::ui::UIEvent::NewLogBatch(batch)).is_err() {
                            // Channel closed, stop the timer.
                            break;
                        }
                    } else {
                        // No sender available, stop the timer.
                        break;
                    }
                }

                // Clean up timer when done.
                *batch_timer.lock().unwrap() = None;
            });
        }
    }
}
